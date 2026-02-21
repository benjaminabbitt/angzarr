"""Process manager for hand flow orchestration."""

import asyncio
from dataclasses import dataclass, field
from datetime import datetime, timezone
from enum import Enum, auto
from typing import Callable, Optional

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import poker_types_pb2 as poker_types


class HandPhase(Enum):
    """Internal state machine phases for hand orchestration."""

    WAITING_FOR_START = auto()
    DEALING = auto()
    POSTING_BLINDS = auto()
    BETTING = auto()
    DEALING_COMMUNITY = auto()
    DRAW = auto()
    SHOWDOWN = auto()
    AWARDING_POT = auto()
    COMPLETE = auto()


@dataclass
class PlayerState:
    """Tracks a player's state within the process manager."""

    player_root: bytes
    position: int
    stack: int
    bet_this_round: int = 0
    total_invested: int = 0
    has_acted: bool = False
    has_folded: bool = False
    is_all_in: bool = False


@dataclass
class HandProcess:
    """
    Process manager state for a single hand.

    This tracks the orchestration state separately from the
    domain state in the hand aggregate. It coordinates:
    - Phase transitions
    - Action timeouts
    - Next player to act
    - Blind posting sequence
    """

    hand_id: str = ""
    table_root: bytes = b""
    hand_number: int = 0
    game_variant: int = 0

    # State machine
    phase: HandPhase = HandPhase.WAITING_FOR_START
    betting_phase: int = poker_types.PREFLOP

    # Player tracking
    players: dict = field(default_factory=dict)  # position -> PlayerState
    active_positions: list = field(default_factory=list)

    # Position tracking
    dealer_position: int = 0
    small_blind_position: int = 0
    big_blind_position: int = 0
    action_on: int = -1
    last_aggressor: int = -1

    # Betting state
    small_blind: int = 0
    big_blind: int = 0
    current_bet: int = 0
    min_raise: int = 0
    pot_total: int = 0

    # Blind posting progress
    small_blind_posted: bool = False
    big_blind_posted: bool = False

    # Timeout handling
    action_timeout_seconds: int = 30
    action_started_at: Optional[datetime] = None

    # Community cards (for phase tracking)
    community_card_count: int = 0


class HandProcessManager:
    """
    Orchestrates the flow of a poker hand.

    The process manager is responsible for:
    1. Determining what action should happen next
    2. Issuing commands to advance the hand
    3. Handling timeouts and auto-actions
    4. Managing phase transitions

    It reacts to events from the hand aggregate and emits
    commands to drive the hand forward.
    """

    def __init__(
        self,
        command_sender: Callable[[types.CommandBook], None],
        timeout_handler: Optional[Callable[[bytes, int], None]] = None,
    ):
        self._command_sender = command_sender
        self._timeout_handler = timeout_handler
        self._processes: dict[str, HandProcess] = {}  # hand_id -> HandProcess
        self._timeout_tasks: dict[str, asyncio.Task] = {}

    def get_process(self, hand_id: str) -> Optional[HandProcess]:
        """Get process state for a hand."""
        return self._processes.get(hand_id)

    def start_hand(
        self,
        event: table.HandStarted,
        table_root: bytes,
        action_timeout: int = 30,
    ) -> HandProcess:
        """
        Initialize process for a new hand.

        Called when HandStarted event is received from table domain.
        """
        hand_id = f"{table_root.hex()}_{event.hand_number}"

        process = HandProcess(
            hand_id=hand_id,
            table_root=table_root,
            hand_number=event.hand_number,
            game_variant=event.game_variant,
            dealer_position=event.dealer_position,
            small_blind_position=event.small_blind_position,
            big_blind_position=event.big_blind_position,
            small_blind=event.small_blind,
            big_blind=event.big_blind,
            action_timeout_seconds=action_timeout,
            phase=HandPhase.DEALING,
        )

        # Initialize player states
        for player in event.active_players:
            process.players[player.position] = PlayerState(
                player_root=player.player_root,
                position=player.position,
                stack=player.stack,
            )
            process.active_positions.append(player.position)

        process.active_positions.sort()
        self._processes[hand_id] = process

        return process

    def handle_cards_dealt(self, hand_id: str, event: hand.CardsDealt):
        """Handle CardsDealt event - transition to blind posting."""
        process = self._processes.get(hand_id)
        if not process:
            return

        process.phase = HandPhase.POSTING_BLINDS
        process.min_raise = process.big_blind

        # Issue post blind commands
        self._post_next_blind(process)

    def handle_blind_posted(self, hand_id: str, event: hand.BlindPosted):
        """Handle BlindPosted event - continue blind posting or start betting."""
        process = self._processes.get(hand_id)
        if not process:
            return

        # Update player state
        for pos, player in process.players.items():
            if player.player_root == event.player_root:
                player.stack = event.player_stack
                player.bet_this_round = event.amount
                player.total_invested = event.amount
                break

        process.pot_total = event.pot_total

        if event.blind_type == "small":
            process.small_blind_posted = True
            process.current_bet = event.amount
            self._post_next_blind(process)
        elif event.blind_type == "big":
            process.big_blind_posted = True
            process.current_bet = event.amount
            self._start_betting(process)

    def handle_action_taken(self, hand_id: str, event: hand.ActionTaken):
        """Handle ActionTaken event - advance to next player or phase."""
        process = self._processes.get(hand_id)
        if not process:
            return

        # Cancel any pending timeout
        self._cancel_timeout(hand_id)

        # Update player state
        for pos, player in process.players.items():
            if player.player_root == event.player_root:
                player.stack = event.player_stack
                player.has_acted = True

                if event.action == poker_types.FOLD:
                    player.has_folded = True
                elif event.action == poker_types.ALL_IN:
                    player.is_all_in = True
                    player.bet_this_round += event.amount
                    player.total_invested += event.amount
                elif event.action in (
                    poker_types.CALL,
                    poker_types.BET,
                    poker_types.RAISE,
                ):
                    player.bet_this_round += event.amount
                    player.total_invested += event.amount

                if event.action in (
                    poker_types.BET,
                    poker_types.RAISE,
                    poker_types.ALL_IN,
                ):
                    if player.bet_this_round > process.current_bet:
                        raise_amount = player.bet_this_round - process.current_bet
                        process.current_bet = player.bet_this_round
                        process.min_raise = max(process.min_raise, raise_amount)
                        process.last_aggressor = pos
                        # Reset has_acted for all other active players
                        for p in process.players.values():
                            if (
                                p.position != pos
                                and not p.has_folded
                                and not p.is_all_in
                            ):
                                p.has_acted = False
                break

        process.pot_total = event.pot_total

        # Check if betting round is complete
        if self._is_betting_complete(process):
            self._end_betting_round(process)
        else:
            # Move to next player
            self._advance_action(process)

    def handle_community_dealt(self, hand_id: str, event: hand.CommunityCardsDealt):
        """Handle CommunityCardsDealt event - start new betting round."""
        process = self._processes.get(hand_id)
        if not process:
            return

        process.community_card_count = len(event.all_community_cards)
        process.betting_phase = event.phase
        self._start_betting(process)

    def handle_pot_awarded(self, hand_id: str, event: hand.PotAwarded):
        """Handle PotAwarded event - hand is complete."""
        process = self._processes.get(hand_id)
        if not process:
            return

        process.phase = HandPhase.COMPLETE
        self._cancel_timeout(hand_id)

    def _post_next_blind(self, process: HandProcess):
        """Post the next required blind."""
        if not process.small_blind_posted:
            player = process.players.get(process.small_blind_position)
            if player:
                self._send_post_blind(process, player, "small", process.small_blind)
        elif not process.big_blind_posted:
            player = process.players.get(process.big_blind_position)
            if player:
                self._send_post_blind(process, player, "big", process.big_blind)

    def _send_post_blind(
        self, process: HandProcess, player: PlayerState, blind_type: str, amount: int
    ):
        """Send PostBlind command."""
        cmd = hand.PostBlind(
            player_root=player.player_root,
            blind_type=blind_type,
            amount=amount,
        )

        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

        hand_root = bytes.fromhex(process.hand_id.split("_")[0])
        self._command_sender(
            types.CommandBook(
                cover=types.Cover(
                    root=types.UUID(value=hand_root),
                    domain="hand",
                ),
                pages=[
                    types.CommandPage(
                        command=cmd_any,
                    )
                ],
            )
        )

    def _start_betting(self, process: HandProcess):
        """Start a new betting round."""
        process.phase = HandPhase.BETTING

        # Reset betting state for new round
        for player in process.players.values():
            player.bet_this_round = 0
            player.has_acted = False

        process.current_bet = 0

        # Determine first to act
        if process.betting_phase == poker_types.PREFLOP:
            # Preflop: UTG (after big blind)
            process.action_on = self._find_next_active(
                process, process.big_blind_position
            )
        else:
            # Postflop: first active player after dealer
            process.action_on = self._find_next_active(process, process.dealer_position)

        self._request_action(process)

    def _advance_action(self, process: HandProcess):
        """Move action to the next player."""
        process.action_on = self._find_next_active(process, process.action_on)
        self._request_action(process)

    def _find_next_active(self, process: HandProcess, after_position: int) -> int:
        """Find the next active player position after the given position."""
        positions = process.active_positions
        n = len(positions)

        if n == 0:
            return -1

        # Find starting index
        start_idx = 0
        for i, pos in enumerate(positions):
            if pos > after_position:
                start_idx = i
                break
        else:
            start_idx = 0  # Wrap around

        # Find next active player
        for i in range(n):
            idx = (start_idx + i) % n
            pos = positions[idx]
            player = process.players.get(pos)
            if player and not player.has_folded and not player.is_all_in:
                return pos

        return -1

    def _is_betting_complete(self, process: HandProcess) -> bool:
        """Check if the current betting round is complete."""
        active_players = [
            p for p in process.players.values() if not p.has_folded and not p.is_all_in
        ]

        if len(active_players) <= 1:
            return True

        # All active players must have acted and matched the bet
        for player in active_players:
            if not player.has_acted:
                return False
            if player.bet_this_round < process.current_bet and not player.is_all_in:
                return False

        return True

    def _end_betting_round(self, process: HandProcess):
        """End the current betting round and advance to next phase."""
        # Count active players
        players_in_hand = [p for p in process.players.values() if not p.has_folded]
        active_players = [p for p in players_in_hand if not p.is_all_in]

        # If only one player left, skip to showdown
        if len(players_in_hand) == 1:
            self._award_pot_to_last_player(process, players_in_hand[0])
            return

        # Determine next phase based on game variant
        if process.game_variant == poker_types.TEXAS_HOLDEM:
            self._advance_holdem_phase(process, len(active_players))
        elif process.game_variant == poker_types.OMAHA:
            self._advance_holdem_phase(process, len(active_players))
        elif process.game_variant == poker_types.FIVE_CARD_DRAW:
            self._advance_draw_phase(process, len(active_players))

    def _advance_holdem_phase(self, process: HandProcess, active_count: int):
        """Advance to next phase for Hold'em/Omaha."""
        if process.betting_phase == poker_types.PREFLOP:
            process.phase = HandPhase.DEALING_COMMUNITY
            self._deal_community(process, 3)  # Flop
        elif process.betting_phase == poker_types.FLOP:
            process.phase = HandPhase.DEALING_COMMUNITY
            self._deal_community(process, 1)  # Turn
        elif process.betting_phase == poker_types.TURN:
            process.phase = HandPhase.DEALING_COMMUNITY
            self._deal_community(process, 1)  # River
        elif process.betting_phase == poker_types.RIVER:
            self._start_showdown(process)

    def _advance_draw_phase(self, process: HandProcess, active_count: int):
        """Advance to next phase for draw games.

        Five Card Draw structure:
        1. PREFLOP betting -> DRAW phase
        2. After draws -> Final betting with betting_phase=DRAW
        3. After final betting -> SHOWDOWN
        """
        if process.betting_phase == poker_types.PREFLOP:
            process.phase = HandPhase.DRAW
            # Draw phase handled by player commands
        elif process.betting_phase == poker_types.DRAW:
            if process.phase == HandPhase.DRAW:
                # Coming from draw phase, start final betting
                process.betting_phase = poker_types.DRAW
                self._start_betting(process)
            else:
                # Coming from final betting, go to showdown
                self._start_showdown(process)

    def _deal_community(self, process: HandProcess, count: int):
        """Send DealCommunityCards command."""
        cmd = hand.DealCommunityCards(count=count)

        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

        hand_root = bytes.fromhex(process.hand_id.split("_")[0])
        self._command_sender(
            types.CommandBook(
                cover=types.Cover(
                    root=types.UUID(value=hand_root),
                    domain="hand",
                ),
                pages=[
                    types.CommandPage(
                        command=cmd_any,
                    )
                ],
            )
        )

    def _start_showdown(self, process: HandProcess):
        """Start the showdown phase."""
        process.phase = HandPhase.SHOWDOWN
        process.betting_phase = poker_types.SHOWDOWN

        # Auto-award to best hand (simplified)
        self._auto_award_pot(process)

    def _award_pot_to_last_player(self, process: HandProcess, winner: PlayerState):
        """Award pot to the last remaining player."""
        process.phase = HandPhase.COMPLETE

        awards = [
            hand.PotAward(
                player_root=winner.player_root,
                amount=process.pot_total,
                pot_type="main",
            )
        ]

        cmd = hand.AwardPot()
        cmd.awards.extend(awards)

        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

        hand_root = bytes.fromhex(process.hand_id.split("_")[0])
        self._command_sender(
            types.CommandBook(
                cover=types.Cover(
                    root=types.UUID(value=hand_root),
                    domain="hand",
                ),
                pages=[
                    types.CommandPage(
                        command=cmd_any,
                    )
                ],
            )
        )

    def _auto_award_pot(self, process: HandProcess):
        """Auto-award pot (simplified - in reality would evaluate hands)."""
        players_in_hand = [p for p in process.players.values() if not p.has_folded]

        if not players_in_hand:
            return

        # For now, split pot evenly (real implementation would evaluate hands)
        split = process.pot_total // len(players_in_hand)
        remainder = process.pot_total % len(players_in_hand)

        awards = []
        for i, player in enumerate(players_in_hand):
            amount = split + (1 if i < remainder else 0)
            awards.append(
                hand.PotAward(
                    player_root=player.player_root,
                    amount=amount,
                    pot_type="main",
                )
            )

        cmd = hand.AwardPot()
        cmd.awards.extend(awards)

        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

        hand_root = bytes.fromhex(process.hand_id.split("_")[0])
        self._command_sender(
            types.CommandBook(
                cover=types.Cover(
                    root=types.UUID(value=hand_root),
                    domain="hand",
                ),
                pages=[
                    types.CommandPage(
                        command=cmd_any,
                    )
                ],
            )
        )

    def _request_action(self, process: HandProcess):
        """Request action from the current player."""
        if process.action_on < 0:
            return

        player = process.players.get(process.action_on)
        if not player:
            return

        process.action_started_at = datetime.now(timezone.utc)

        # Start timeout timer
        self._start_timeout(process.hand_id, player.player_root, process.action_on)

    def _start_timeout(self, hand_id: str, player_root: bytes, position: int):
        """Start action timeout for a player."""
        process = self._processes.get(hand_id)
        if not process:
            return

        # In async context, would create a task
        # For now, just track the start time
        # Actual timeout handling would be done by external timer service

    def _cancel_timeout(self, hand_id: str):
        """Cancel any pending timeout for this hand."""
        if hand_id in self._timeout_tasks:
            self._timeout_tasks[hand_id].cancel()
            del self._timeout_tasks[hand_id]

    def handle_timeout(self, hand_id: str, position: int):
        """Handle action timeout - auto-fold or auto-check."""
        process = self._processes.get(hand_id)
        if not process or process.action_on != position:
            return

        player = process.players.get(position)
        if not player:
            return

        # Determine default action
        call_amount = process.current_bet - player.bet_this_round
        if call_amount == 0:
            default_action = poker_types.CHECK
            amount = 0
        else:
            default_action = poker_types.FOLD
            amount = 0

        # Send the default action
        cmd = hand.PlayerAction(
            player_root=player.player_root,
            action=default_action,
            amount=amount,
        )

        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

        hand_root = bytes.fromhex(process.hand_id.split("_")[0])
        self._command_sender(
            types.CommandBook(
                cover=types.Cover(
                    root=types.UUID(value=hand_root),
                    domain="hand",
                ),
                pages=[
                    types.CommandPage(
                        command=cmd_any,
                    )
                ],
            )
        )
