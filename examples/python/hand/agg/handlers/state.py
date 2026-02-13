"""Hand aggregate state management."""

import sys
from dataclasses import dataclass, field
from pathlib import Path
from typing import Optional

from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import types_pb2 as poker_types


@dataclass
class PlayerHandInfo:
    """State for a player in the hand."""

    player_root: bytes = b""
    position: int = 0
    hole_cards: list = field(default_factory=list)  # List of (suit, rank) tuples
    stack: int = 0
    bet_this_round: int = 0
    total_invested: int = 0
    has_acted: bool = False
    has_folded: bool = False
    is_all_in: bool = False


@dataclass
class PotInfo:
    """State for a pot."""

    amount: int = 0
    eligible_players: list = field(default_factory=list)  # List of player_root bytes
    pot_type: str = "main"


@dataclass
class HandState:
    """Complete state for a hand aggregate."""

    hand_id: str = ""
    table_root: bytes = b""
    hand_number: int = 0
    game_variant: int = 0

    # Deck state - list of (suit, rank) tuples
    remaining_deck: list = field(default_factory=list)

    # Player state - position -> PlayerHandInfo
    players: dict = field(default_factory=dict)

    # Community cards - list of (suit, rank) tuples
    community_cards: list = field(default_factory=list)

    # Betting state
    current_phase: int = 0  # BettingPhase enum
    action_on_position: int = -1
    current_bet: int = 0
    min_raise: int = 0
    pots: list = field(default_factory=list)  # List of PotInfo

    # Positions
    dealer_position: int = 0
    small_blind_position: int = 0
    big_blind_position: int = 0
    small_blind: int = 0
    big_blind: int = 0

    status: str = ""  # "dealing", "betting", "showdown", "complete"

    def exists(self) -> bool:
        """Check if hand has been dealt."""
        return self.status != ""

    def get_active_players(self) -> list:
        """Get players still in the hand (not folded, not all-in)."""
        return [
            p for p in self.players.values() if not p.has_folded and not p.is_all_in
        ]

    def get_players_in_hand(self) -> list:
        """Get all players still in hand (not folded)."""
        return [p for p in self.players.values() if not p.has_folded]

    def get_next_to_act(self) -> Optional[int]:
        """Find next position that needs to act."""
        active = self.get_active_players()
        if len(active) <= 1:
            return None

        positions = sorted(p.position for p in active)
        if not positions:
            return None

        # Find next position after current
        for pos in positions:
            if pos > self.action_on_position:
                player = self.players.get(pos)
                if player and not player.has_acted:
                    return pos

        # Wrap around
        for pos in positions:
            player = self.players.get(pos)
            if player and not player.has_acted:
                return pos

        return None

    def is_betting_complete(self) -> bool:
        """Check if current betting round is complete."""
        active = self.get_active_players()
        if len(active) <= 1:
            return True

        # All active players must have acted and matched current bet
        for player in active:
            if not player.has_acted:
                return False
            if player.bet_this_round < self.current_bet and not player.is_all_in:
                return False

        return True

    def get_pot_total(self) -> int:
        """Get total pot amount."""
        return sum(p.amount for p in self.pots)


def rebuild_state(event_book: types.EventBook) -> HandState:
    """Rebuild hand state from event history."""
    state = HandState()

    if not event_book.pages:
        return state

    # Sort pages by sequence number to ensure correct order
    def get_seq(p):
        if p.WhichOneof('sequence') == 'num':
            return p.num
        return 0
    sorted_pages = sorted(event_book.pages, key=get_seq)

    for page in sorted_pages:
        event_any = page.event
        type_url = event_any.type_url

        # NOTE: Check CommunityCardsDealt BEFORE CardsDealt since "CardsDealt" is a substring
        if "CommunityCardsDealt" in type_url:
            event = hand.CommunityCardsDealt()
            event_any.Unpack(event)

            for card in event.cards:
                dealt_card = (card.suit, card.rank)
                state.community_cards.append(dealt_card)
                # Remove from remaining deck
                if dealt_card in state.remaining_deck:
                    state.remaining_deck.remove(dealt_card)

            state.current_phase = event.phase
            state.status = "betting"  # Ready for next betting round

            # Reset for next betting round
            for player in state.players.values():
                player.bet_this_round = 0
                player.has_acted = False
            state.current_bet = 0

        elif "CardsDealt" in type_url:
            event = hand.CardsDealt()
            event_any.Unpack(event)
            state.hand_id = f"{event.table_root.hex()}_{event.hand_number}"
            state.table_root = event.table_root
            state.hand_number = event.hand_number
            state.game_variant = event.game_variant
            state.dealer_position = event.dealer_position
            state.status = "betting"
            state.current_phase = poker_types.PREFLOP

            # Initialize players
            for player in event.players:
                state.players[player.position] = PlayerHandInfo(
                    player_root=player.player_root,
                    position=player.position,
                    stack=player.stack,
                )

            # Collect all dealt cards
            dealt_cards = set()

            # Set hole cards
            for pc in event.player_cards:
                if pc.player_root in [p.player_root for p in state.players.values()]:
                    for pos, player in state.players.items():
                        if player.player_root == pc.player_root:
                            player.hole_cards = [(c.suit, c.rank) for c in pc.cards]
                            for c in pc.cards:
                                dealt_cards.add((c.suit, c.rank))

            # Reconstruct remaining deck (all cards not dealt)
            full_deck = []
            for suit in [
                poker_types.CLUBS,
                poker_types.DIAMONDS,
                poker_types.HEARTS,
                poker_types.SPADES,
            ]:
                for rank in range(2, 15):
                    card = (suit, rank)
                    if card not in dealt_cards:
                        full_deck.append(card)
            # Shuffle the remaining deck to randomize community card order
            import random
            random.shuffle(full_deck)
            state.remaining_deck = full_deck

            # Initialize main pot
            state.pots = [
                PotInfo(
                    amount=0,
                    eligible_players=[p.player_root for p in state.players.values()],
                    pot_type="main",
                )
            ]

        elif "BlindPosted" in type_url:
            event = hand.BlindPosted()
            event_any.Unpack(event)

            for player in state.players.values():
                if player.player_root == event.player_root:
                    player.stack = event.player_stack
                    player.bet_this_round = event.amount
                    player.total_invested += event.amount
                    if event.blind_type == "small":
                        state.small_blind_position = player.position
                        state.small_blind = event.amount
                    elif event.blind_type == "big":
                        state.big_blind_position = player.position
                        state.big_blind = event.amount
                        state.current_bet = event.amount
                        state.min_raise = event.amount
                    break

            if state.pots:
                state.pots[0].amount = event.pot_total

            state.status = "betting"

        elif "ActionTaken" in type_url:
            event = hand.ActionTaken()
            event_any.Unpack(event)

            for player in state.players.values():
                if player.player_root == event.player_root:
                    player.stack = event.player_stack
                    player.has_acted = True

                    if event.action == poker_types.FOLD:
                        player.has_folded = True
                    elif event.action in (
                        poker_types.CALL,
                        poker_types.BET,
                        poker_types.RAISE,
                    ):
                        player.bet_this_round += event.amount
                        player.total_invested += event.amount
                    elif event.action == poker_types.ALL_IN:
                        player.is_all_in = True
                        player.bet_this_round += event.amount
                        player.total_invested += event.amount

                    if event.action in (
                        poker_types.BET,
                        poker_types.RAISE,
                        poker_types.ALL_IN,
                    ):
                        if player.bet_this_round > state.current_bet:
                            raise_amount = player.bet_this_round - state.current_bet
                            state.current_bet = player.bet_this_round
                            state.min_raise = max(state.min_raise, raise_amount)
                    break

            if state.pots:
                state.pots[0].amount = event.pot_total

            state.action_on_position = -1  # Will be set by process manager

        elif "BettingRoundComplete" in type_url:
            event = hand.BettingRoundComplete()
            event_any.Unpack(event)

            # Reset for next round
            for player in state.players.values():
                player.bet_this_round = 0
                player.has_acted = False

            state.current_bet = 0
            if state.pots:
                state.pots[0].amount = event.pot_total

            # Advance phase for draw-based games (Five Card Draw)
            # Community card games advance phase when cards are dealt
            from .game_rules import get_game_rules
            rules = get_game_rules(state.game_variant)
            if rules.variant == poker_types.FIVE_CARD_DRAW:
                transition = rules.get_next_phase(event.completed_phase)
                if transition:
                    state.current_phase = transition.next_phase

        elif "DrawCompleted" in type_url:
            event = hand.DrawCompleted()
            event_any.Unpack(event)

            for player in state.players.values():
                if player.player_root == event.player_root:
                    # Remove discarded cards and add new ones
                    # For now, just update with new cards
                    player.hole_cards = [(c.suit, c.rank) for c in event.new_cards]
                    break

        elif "ShowdownStarted" in type_url:
            state.status = "showdown"

        elif "CardsRevealed" in type_url:
            pass  # Just for display

        elif "PotAwarded" in type_url:
            event = hand.PotAwarded()
            event_any.Unpack(event)

            for winner in event.winners:
                for player in state.players.values():
                    if player.player_root == winner.player_root:
                        player.stack += winner.amount
                        break

        elif "HandComplete" in type_url:
            state.status = "complete"

    return state
