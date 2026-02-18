"""Hand aggregate - rich domain model."""

import random
from dataclasses import dataclass, field
from typing import Optional, Union, Tuple

from angzarr_client import Aggregate, handles, now
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import hand_pb2 as hand_proto
from angzarr_client.proto.examples import types_pb2 as poker_types

from .game_rules import get_game_rules


@dataclass
class _PlayerHandInfo:
    """State for a player in the hand."""
    player_root: bytes = b""
    position: int = 0
    hole_cards: list = field(default_factory=list)
    stack: int = 0
    bet_this_round: int = 0
    total_invested: int = 0
    has_acted: bool = False
    has_folded: bool = False
    is_all_in: bool = False


@dataclass
class _PotInfo:
    """State for a pot."""
    amount: int = 0
    eligible_players: list = field(default_factory=list)
    pot_type: str = "main"


@dataclass
class _HandState:
    """Internal state representation."""
    hand_id: str = ""
    table_root: bytes = b""
    hand_number: int = 0
    game_variant: int = 0
    remaining_deck: list = field(default_factory=list)
    players: dict = field(default_factory=dict)
    community_cards: list = field(default_factory=list)
    current_phase: int = 0
    action_on_position: int = -1
    current_bet: int = 0
    min_raise: int = 0
    pots: list = field(default_factory=list)
    dealer_position: int = 0
    small_blind_position: int = 0
    big_blind_position: int = 0
    small_blind: int = 0
    big_blind: int = 0
    status: str = ""


class Hand(Aggregate[_HandState]):
    """Hand aggregate with event sourcing."""

    domain = "hand"

    def _create_empty_state(self) -> _HandState:
        return _HandState()

    def _apply_event(self, state: _HandState, event_any) -> None:
        """Apply a single event to state."""
        type_url = event_any.type_url

        if "CommunityCardsDealt" in type_url:
            event = hand_proto.CommunityCardsDealt()
            event_any.Unpack(event)
            for card in event.cards:
                dealt_card = (card.suit, card.rank)
                state.community_cards.append(dealt_card)
                if dealt_card in state.remaining_deck:
                    state.remaining_deck.remove(dealt_card)
            state.current_phase = event.phase
            state.status = "betting"
            for player in state.players.values():
                player.bet_this_round = 0
                player.has_acted = False
            state.current_bet = 0

        elif "CardsDealt" in type_url:
            event = hand_proto.CardsDealt()
            event_any.Unpack(event)
            state.hand_id = f"{event.table_root.hex()}_{event.hand_number}"
            state.table_root = event.table_root
            state.hand_number = event.hand_number
            state.game_variant = event.game_variant
            state.dealer_position = event.dealer_position
            state.status = "betting"
            state.current_phase = poker_types.PREFLOP

            for player in event.players:
                state.players[player.position] = _PlayerHandInfo(
                    player_root=player.player_root,
                    position=player.position,
                    stack=player.stack,
                )

            dealt_cards = set()
            for pc in event.player_cards:
                for pos, player in state.players.items():
                    if player.player_root == pc.player_root:
                        player.hole_cards = [(c.suit, c.rank) for c in pc.cards]
                        for c in pc.cards:
                            dealt_cards.add((c.suit, c.rank))

            full_deck = []
            for suit in [poker_types.CLUBS, poker_types.DIAMONDS,
                         poker_types.HEARTS, poker_types.SPADES]:
                for rank in range(2, 15):
                    card = (suit, rank)
                    if card not in dealt_cards:
                        full_deck.append(card)
            random.shuffle(full_deck)
            state.remaining_deck = full_deck

            state.pots = [_PotInfo(
                amount=0,
                eligible_players=[p.player_root for p in state.players.values()],
                pot_type="main",
            )]

        elif "BlindPosted" in type_url:
            event = hand_proto.BlindPosted()
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
            event = hand_proto.ActionTaken()
            event_any.Unpack(event)
            for player in state.players.values():
                if player.player_root == event.player_root:
                    player.stack = event.player_stack
                    player.has_acted = True
                    if event.action == poker_types.FOLD:
                        player.has_folded = True
                    elif event.action in (poker_types.CALL, poker_types.BET, poker_types.RAISE):
                        player.bet_this_round += event.amount
                        player.total_invested += event.amount
                    elif event.action == poker_types.ALL_IN:
                        player.is_all_in = True
                        player.bet_this_round += event.amount
                        player.total_invested += event.amount
                    if event.action in (poker_types.BET, poker_types.RAISE, poker_types.ALL_IN):
                        if player.bet_this_round > state.current_bet:
                            raise_amount = player.bet_this_round - state.current_bet
                            state.current_bet = player.bet_this_round
                            state.min_raise = max(state.min_raise, raise_amount)
                    break
            if state.pots:
                state.pots[0].amount = event.pot_total
            state.action_on_position = -1

        elif "ShowdownStarted" in type_url:
            state.status = "showdown"

        elif "DrawCompleted" in type_url:
            event = hand_proto.DrawCompleted()
            event_any.Unpack(event)
            for player in state.players.values():
                if player.player_root == event.player_root:
                    # Remove discarded cards and add new ones
                    new_cards = [(c.suit, c.rank) for c in event.new_cards]
                    # For simplicity, replace the specified number of cards
                    if event.cards_discarded > 0:
                        player.hole_cards = player.hole_cards[event.cards_discarded:]
                    player.hole_cards.extend(new_cards)
                    # Remove dealt cards from deck
                    for card in new_cards:
                        if card in state.remaining_deck:
                            state.remaining_deck.remove(card)
                    break

        elif "PotAwarded" in type_url:
            event = hand_proto.PotAwarded()
            event_any.Unpack(event)
            for winner in event.winners:
                for player in state.players.values():
                    if player.player_root == winner.player_root:
                        player.stack += winner.amount
                        break

        elif "HandComplete" in type_url:
            state.status = "complete"

    # --- State accessors ---

    @property
    def exists(self) -> bool:
        return self._get_state().status != ""

    @property
    def hand_id(self) -> str:
        return self._get_state().hand_id

    @property
    def table_root(self) -> bytes:
        return self._get_state().table_root

    @property
    def hand_number(self) -> int:
        return self._get_state().hand_number

    @property
    def game_variant(self) -> int:
        return self._get_state().game_variant

    @property
    def status(self) -> str:
        return self._get_state().status

    @property
    def current_phase(self) -> int:
        return self._get_state().current_phase

    @property
    def current_bet(self) -> int:
        return self._get_state().current_bet

    @property
    def min_raise(self) -> int:
        return self._get_state().min_raise

    @property
    def small_blind(self) -> int:
        return self._get_state().small_blind

    @property
    def big_blind(self) -> int:
        return self._get_state().big_blind

    @property
    def community_cards(self) -> list:
        return self._get_state().community_cards

    @property
    def players(self) -> dict:
        return self._get_state().players

    @property
    def remaining_deck(self) -> list:
        return self._get_state().remaining_deck

    def get_pot_total(self) -> int:
        return sum(p.amount for p in self._get_state().pots)

    def get_player(self, player_root: bytes) -> Optional[_PlayerHandInfo]:
        for p in self._get_state().players.values():
            if p.player_root == player_root:
                return p
        return None

    def get_active_players(self) -> list:
        return [p for p in self._get_state().players.values()
                if not p.has_folded and not p.is_all_in]

    def get_players_in_hand(self) -> list:
        return [p for p in self._get_state().players.values() if not p.has_folded]

    # --- Command handlers ---

    @handles(hand_proto.DealCards)
    def deal(self, cmd: hand_proto.DealCards) -> hand_proto.CardsDealt:
        """Deal cards to start the hand."""
        if self.exists:
            raise CommandRejectedError("Hand already dealt")
        if not cmd.players:
            raise CommandRejectedError("No players in hand")
        if len(cmd.players) < 2:
            raise CommandRejectedError("Need at least 2 players")

        rules = get_game_rules(cmd.game_variant)
        player_roots = [p.player_root for p in cmd.players]
        deal_result = rules.deal_hole_cards(
            deck=[],
            players=player_roots,
            seed=cmd.deck_seed if cmd.deck_seed else None,
        )

        player_cards = []
        for player_root, cards in deal_result.player_cards.items():
            pc = hand_proto.PlayerHoleCards(player_root=player_root)
            for suit, rank in cards:
                pc.cards.append(poker_types.Card(suit=suit, rank=rank))
            player_cards.append(pc)

        event = hand_proto.CardsDealt(
            table_root=cmd.table_root,
            hand_number=cmd.hand_number,
            game_variant=cmd.game_variant,
            dealer_position=cmd.dealer_position,
            dealt_at=now(),
        )
        event.player_cards.extend(player_cards)
        event.players.extend(cmd.players)

        return event

    @handles(hand_proto.PostBlind)
    def post_blind(self, cmd: hand_proto.PostBlind) -> hand_proto.BlindPosted:
        """Post a blind."""
        if not self.exists:
            raise CommandRejectedError("Hand not dealt")
        if self.status == "complete":
            raise CommandRejectedError("Hand is complete")
        if not cmd.player_root:
            raise CommandRejectedError("player_root is required")

        player = self.get_player(cmd.player_root)
        if not player:
            raise CommandRejectedError("Player not in hand")
        if player.has_folded:
            raise CommandRejectedError("Player has folded")
        if cmd.amount <= 0:
            raise CommandRejectedError("Blind amount must be positive")

        actual_amount = min(cmd.amount, player.stack)
        new_stack = player.stack - actual_amount
        new_pot_total = self.get_pot_total() + actual_amount

        return hand_proto.BlindPosted(
            player_root=cmd.player_root,
            blind_type=cmd.blind_type,
            amount=actual_amount,
            player_stack=new_stack,
            pot_total=new_pot_total,
            posted_at=now(),
        )

    @handles(hand_proto.PlayerAction)
    def action(self, cmd: hand_proto.PlayerAction) -> hand_proto.ActionTaken:
        """Process a player action."""
        if not self.exists:
            raise CommandRejectedError("Hand not dealt")
        if self.status != "betting":
            raise CommandRejectedError("Not in betting phase")
        if not cmd.player_root:
            raise CommandRejectedError("player_root is required")

        player = self.get_player(cmd.player_root)
        if not player:
            raise CommandRejectedError("Player not in hand")
        if player.has_folded:
            raise CommandRejectedError("Player has folded")
        if player.is_all_in:
            raise CommandRejectedError("Player is all-in")

        action = cmd.action
        amount = cmd.amount
        call_amount = self.current_bet - player.bet_this_round

        if action == poker_types.FOLD:
            amount = 0
        elif action == poker_types.CHECK:
            if call_amount > 0:
                raise CommandRejectedError("Cannot check when there is a bet to call")
            amount = 0
        elif action == poker_types.CALL:
            if call_amount == 0:
                raise CommandRejectedError("Nothing to call")
            actual_amount = min(call_amount, player.stack)
            amount = actual_amount
            if player.stack - actual_amount == 0:
                action = poker_types.ALL_IN
        elif action == poker_types.BET:
            if self.current_bet > 0:
                raise CommandRejectedError("Cannot bet when there is already a bet")
            if amount < self.big_blind:
                raise CommandRejectedError(f"Bet must be at least {self.big_blind}")
            if amount > player.stack:
                raise CommandRejectedError("Bet exceeds stack")
            if player.stack - amount == 0:
                action = poker_types.ALL_IN
        elif action == poker_types.RAISE:
            if self.current_bet == 0:
                raise CommandRejectedError("Cannot raise when there is no bet")
            total_bet = player.bet_this_round + amount
            raise_amount = total_bet - self.current_bet
            if raise_amount < self.min_raise and amount < player.stack:
                raise CommandRejectedError(f"Raise must be at least {self.min_raise}")
            if amount > player.stack:
                raise CommandRejectedError("Raise exceeds stack")
            if player.stack - amount == 0:
                action = poker_types.ALL_IN
        elif action == poker_types.ALL_IN:
            amount = player.stack
        else:
            raise CommandRejectedError("Invalid action")

        new_stack = player.stack - amount
        new_pot_total = self.get_pot_total() + amount

        return hand_proto.ActionTaken(
            player_root=cmd.player_root,
            action=action,
            amount=amount,
            player_stack=new_stack,
            pot_total=new_pot_total,
            amount_to_call=max(self.current_bet, player.bet_this_round + amount) - player.bet_this_round,
            action_at=now(),
        )

    @handles(hand_proto.DealCommunityCards)
    def deal_community(self, cmd: hand_proto.DealCommunityCards) -> hand_proto.CommunityCardsDealt:
        """Deal community cards."""
        if not self.exists:
            raise CommandRejectedError("Hand not dealt")
        if self.status == "complete":
            raise CommandRejectedError("Hand is complete")
        if cmd.count <= 0:
            raise CommandRejectedError("Must deal at least 1 card")

        state = self._get_state()
        rules = get_game_rules(state.game_variant)

        if rules.variant == poker_types.FIVE_CARD_DRAW:
            raise CommandRejectedError("Five card draw doesn't have community cards")

        transition = rules.get_next_phase(state.current_phase)
        if not transition:
            raise CommandRejectedError("No more phases")
        if transition.community_cards_to_deal != cmd.count:
            raise CommandRejectedError(
                f"Expected {transition.community_cards_to_deal} cards for this phase"
            )
        if len(state.remaining_deck) < cmd.count:
            raise CommandRejectedError("Not enough cards in deck")

        new_cards = state.remaining_deck[:cmd.count]
        all_community = state.community_cards + new_cards

        event = hand_proto.CommunityCardsDealt(
            phase=transition.next_phase,
            dealt_at=now(),
        )
        for suit, rank in new_cards:
            event.cards.append(poker_types.Card(suit=suit, rank=rank))
        for suit, rank in all_community:
            event.all_community_cards.append(poker_types.Card(suit=suit, rank=rank))

        return event

    @handles(hand_proto.RequestDraw)
    def draw(self, cmd: hand_proto.RequestDraw) -> hand_proto.DrawCompleted:
        """Handle draw request for Five Card Draw."""
        if not self.exists:
            raise CommandRejectedError("Hand not dealt")
        if self.status == "complete":
            raise CommandRejectedError("Hand is complete")
        if not cmd.player_root:
            raise CommandRejectedError("player_root is required")

        player = self.get_player(cmd.player_root)
        if not player:
            raise CommandRejectedError("Player not in hand")
        if player.has_folded:
            raise CommandRejectedError("Player has folded")

        state = self._get_state()
        if state.game_variant != poker_types.FIVE_CARD_DRAW:
            raise CommandRejectedError("Draw not supported in this game variant")

        # Validate indices
        indices = list(cmd.card_indices)
        if len(indices) > 5:
            raise CommandRejectedError("Cannot discard more than 5 cards")
        for idx in indices:
            if idx < 0 or idx >= len(player.hole_cards):
                raise CommandRejectedError(f"Invalid card index: {idx}")

        # Draw new cards from deck
        cards_to_draw = len(indices)
        if len(state.remaining_deck) < cards_to_draw:
            raise CommandRejectedError("Not enough cards in deck")

        new_cards = state.remaining_deck[:cards_to_draw]

        event = hand_proto.DrawCompleted(
            player_root=cmd.player_root,
            cards_discarded=len(indices),
            cards_drawn=cards_to_draw,
            drawn_at=now(),
        )
        for suit, rank in new_cards:
            event.new_cards.append(poker_types.Card(suit=suit, rank=rank))

        return event

    @handles(hand_proto.RevealCards)
    def reveal(self, cmd: hand_proto.RevealCards) -> Union[hand_proto.CardsRevealed, hand_proto.CardsMucked]:
        """Reveal or muck cards at showdown."""
        if not self.exists:
            raise CommandRejectedError("Hand not dealt")
        if self.status != "showdown":
            raise CommandRejectedError("Not in showdown")
        if not cmd.player_root:
            raise CommandRejectedError("player_root is required")

        player = self.get_player(cmd.player_root)
        if not player:
            raise CommandRejectedError("Player not in hand")
        if player.has_folded:
            raise CommandRejectedError("Player has folded")

        if cmd.muck:
            return hand_proto.CardsMucked(
                player_root=cmd.player_root,
                mucked_at=now(),
            )

        state = self._get_state()
        rules = get_game_rules(state.game_variant)
        rank_type, score, kickers = rules.evaluate_hand(
            player.hole_cards,
            state.community_cards,
        )

        event = hand_proto.CardsRevealed(
            player_root=cmd.player_root,
            ranking=poker_types.HandRanking(
                rank_type=rank_type,
                kickers=[k for k in kickers],
                score=score,
            ),
            revealed_at=now(),
        )
        for suit, rank in player.hole_cards:
            event.cards.append(poker_types.Card(suit=suit, rank=rank))

        return event

    @handles(hand_proto.AwardPot)
    def award(self, cmd: hand_proto.AwardPot) -> Tuple[hand_proto.PotAwarded, hand_proto.HandComplete]:
        """Award pot and complete the hand."""
        if not self.exists:
            raise CommandRejectedError("Hand not dealt")
        if self.status == "complete":
            raise CommandRejectedError("Hand already complete")
        if not cmd.awards:
            raise CommandRejectedError("No awards specified")

        state = self._get_state()

        for award in cmd.awards:
            player = self.get_player(award.player_root)
            if not player:
                raise CommandRejectedError("Winner not in hand")
            if player.has_folded:
                raise CommandRejectedError("Folded player cannot win pot")

        # Adjust awards to match pot if needed
        total_awarded = sum(a.amount for a in cmd.awards)
        pot_total = self.get_pot_total()
        awards = list(cmd.awards)
        if total_awarded != pot_total and pot_total > 0 and len(awards) > 0:
            awards[0].amount = pot_total - sum(a.amount for a in awards[1:])

        winners = []
        for award in awards:
            winners.append(hand_proto.PotWinner(
                player_root=award.player_root,
                amount=award.amount,
                pot_type=award.pot_type,
            ))

        pot_event = hand_proto.PotAwarded(awarded_at=now())
        pot_event.winners.extend(winners)

        final_stacks = []
        for player in state.players.values():
            player_amount = sum(a.amount for a in awards if a.player_root == player.player_root)
            final_stacks.append(hand_proto.PlayerStackSnapshot(
                player_root=player.player_root,
                stack=player.stack + player_amount,
                is_all_in=player.is_all_in,
                has_folded=player.has_folded,
            ))

        complete_event = hand_proto.HandComplete(
            table_root=state.table_root,
            hand_number=state.hand_number,
            completed_at=now(),
        )
        complete_event.winners.extend(winners)
        complete_event.final_stacks.extend(final_stacks)

        return pot_event, complete_event
