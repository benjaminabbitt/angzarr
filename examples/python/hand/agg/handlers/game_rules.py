"""Polymorphic game rules for different poker variants."""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import Optional
import random

from angzarr_client.proto.examples import poker_types_pb2 as poker_types


@dataclass
class DealResult:
    """Result of dealing cards."""

    player_cards: dict  # player_root -> list of (suit, rank) tuples
    remaining_deck: list  # Remaining cards in deck


@dataclass
class DrawResult:
    """Result of executing a draw."""

    new_hole_cards: list  # Updated hole cards after draw
    cards_drawn: list  # New cards drawn from deck
    remaining_deck: list  # Remaining cards in deck


@dataclass
class PhaseTransition:
    """Information about phase transition."""

    next_phase: int  # BettingPhase enum
    community_cards_to_deal: int
    is_showdown: bool


class GameRules(ABC):
    """Abstract base class for poker variant rules."""

    @property
    @abstractmethod
    def variant(self) -> int:
        """Return the GameVariant enum value."""
        pass

    @property
    @abstractmethod
    def hole_card_count(self) -> int:
        """Number of hole cards dealt to each player."""
        pass

    @property
    @abstractmethod
    def phases(self) -> list:
        """List of betting phases for this variant."""
        pass

    @abstractmethod
    def get_next_phase(self, current_phase: int) -> Optional[PhaseTransition]:
        """Get the next phase after the current one, or None if hand is complete."""
        pass

    @abstractmethod
    def evaluate_hand(self, hole_cards: list, community_cards: list) -> tuple:
        """
        Evaluate a hand and return (rank_type, score, kickers).

        rank_type: HandRankType enum
        score: Numeric score for comparison
        kickers: List of Rank values for tie-breaking
        """
        pass

    def create_deck(self, seed: Optional[bytes] = None) -> list:
        """Create and shuffle a standard 52-card deck."""
        deck = []
        for suit in [
            poker_types.CLUBS,
            poker_types.DIAMONDS,
            poker_types.HEARTS,
            poker_types.SPADES,
        ]:
            for rank in range(2, 15):  # 2-14 (Ace)
                deck.append((suit, rank))

        if seed:
            rng = random.Random(int.from_bytes(seed[:8], "big"))
            rng.shuffle(deck)
        else:
            random.shuffle(deck)

        return deck

    def deal_hole_cards(
        self, deck: list, players: list, seed: Optional[bytes] = None
    ) -> DealResult:
        """Deal hole cards to all players."""
        if seed:
            working_deck = self.create_deck(seed)
        elif not deck:
            working_deck = self.create_deck()
        else:
            working_deck = deck.copy()

        player_cards = {}
        for player_root in players:
            cards = []
            for _ in range(self.hole_card_count):
                if working_deck:
                    cards.append(working_deck.pop())
            player_cards[player_root] = cards

        return DealResult(
            player_cards=player_cards,
            remaining_deck=working_deck,
        )


class TexasHoldemRules(GameRules):
    """Rules for Texas Hold'em poker."""

    @property
    def variant(self) -> int:
        return poker_types.TEXAS_HOLDEM

    @property
    def hole_card_count(self) -> int:
        return 2

    @property
    def phases(self) -> list:
        return [
            poker_types.PREFLOP,
            poker_types.FLOP,
            poker_types.TURN,
            poker_types.RIVER,
            poker_types.SHOWDOWN,
        ]

    def get_next_phase(self, current_phase: int) -> Optional[PhaseTransition]:
        if current_phase == poker_types.PREFLOP:
            return PhaseTransition(
                next_phase=poker_types.FLOP,
                community_cards_to_deal=3,
                is_showdown=False,
            )
        elif current_phase == poker_types.FLOP:
            return PhaseTransition(
                next_phase=poker_types.TURN,
                community_cards_to_deal=1,
                is_showdown=False,
            )
        elif current_phase == poker_types.TURN:
            return PhaseTransition(
                next_phase=poker_types.RIVER,
                community_cards_to_deal=1,
                is_showdown=False,
            )
        elif current_phase == poker_types.RIVER:
            return PhaseTransition(
                next_phase=poker_types.SHOWDOWN,
                community_cards_to_deal=0,
                is_showdown=True,
            )
        return None

    def evaluate_hand(self, hole_cards: list, community_cards: list) -> tuple:
        """Evaluate best 5-card hand from 7 cards (2 hole + 5 community)."""
        all_cards = hole_cards + community_cards
        return self._find_best_hand(all_cards)

    def _find_best_hand(self, cards: list) -> tuple:
        """Find the best 5-card hand from available cards."""
        if len(cards) < 5:
            return (poker_types.HIGH_CARD, 0, [])

        from itertools import combinations

        best = (poker_types.HIGH_CARD, 0, [])
        for combo in combinations(cards, 5):
            result = self._evaluate_five(list(combo))
            if result[1] > best[1]:
                best = result

        return best

    def _evaluate_five(self, cards: list) -> tuple:
        """Evaluate exactly 5 cards."""
        suits = [c[0] for c in cards]
        ranks = sorted([c[1] for c in cards], reverse=True)

        is_flush = len(set(suits)) == 1
        is_straight = self._is_straight(ranks)

        rank_counts = {}
        for r in ranks:
            rank_counts[r] = rank_counts.get(r, 0) + 1

        counts = sorted(rank_counts.values(), reverse=True)
        sorted_by_count = sorted(
            rank_counts.keys(), key=lambda x: (rank_counts[x], x), reverse=True
        )

        # Calculate score
        if is_straight and is_flush:
            if ranks == [14, 13, 12, 11, 10]:
                return (poker_types.ROYAL_FLUSH, 10000000, [])
            return (poker_types.STRAIGHT_FLUSH, 9000000 + ranks[0], [])
        elif counts == [4, 1]:
            return (
                poker_types.FOUR_OF_A_KIND,
                8000000 + sorted_by_count[0] * 100,
                [sorted_by_count[1]],
            )
        elif counts == [3, 2]:
            return (
                poker_types.FULL_HOUSE,
                7000000 + sorted_by_count[0] * 100 + sorted_by_count[1],
                [],
            )
        elif is_flush:
            return (poker_types.FLUSH, 6000000 + self._rank_score(ranks), ranks)
        elif is_straight:
            return (poker_types.STRAIGHT, 5000000 + ranks[0], [])
        elif counts == [3, 1, 1]:
            kickers = [r for r in sorted_by_count if rank_counts[r] == 1]
            return (
                poker_types.THREE_OF_A_KIND,
                4000000 + sorted_by_count[0] * 1000,
                kickers,
            )
        elif counts == [2, 2, 1]:
            pairs = [r for r in sorted_by_count if rank_counts[r] == 2]
            kicker = [r for r in sorted_by_count if rank_counts[r] == 1]
            return (
                poker_types.TWO_PAIR,
                3000000 + max(pairs) * 100 + min(pairs),
                kicker,
            )
        elif counts == [2, 1, 1, 1]:
            pair = [r for r in sorted_by_count if rank_counts[r] == 2][0]
            kickers = [r for r in sorted_by_count if rank_counts[r] == 1]
            return (poker_types.PAIR, 2000000 + pair * 1000, kickers)
        else:
            return (poker_types.HIGH_CARD, 1000000 + self._rank_score(ranks), ranks)

    def _is_straight(self, ranks: list) -> bool:
        """Check if sorted ranks form a straight."""
        if ranks == [14, 5, 4, 3, 2]:  # Wheel (A-2-3-4-5)
            return True
        for i in range(len(ranks) - 1):
            if ranks[i] - ranks[i + 1] != 1:
                return False
        return True

    def _rank_score(self, ranks: list) -> int:
        """Calculate a score from ranks for comparison."""
        score = 0
        for i, r in enumerate(ranks):
            score += r * (15 ** (4 - i))
        return score


class OmahaRules(TexasHoldemRules):
    """Rules for Omaha poker (uses 2 of 4 hole cards)."""

    @property
    def variant(self) -> int:
        return poker_types.OMAHA

    @property
    def hole_card_count(self) -> int:
        return 4

    def evaluate_hand(self, hole_cards: list, community_cards: list) -> tuple:
        """Evaluate best hand using exactly 2 hole cards and 3 community cards."""
        from itertools import combinations

        best = (poker_types.HIGH_CARD, 0, [])

        # Must use exactly 2 hole cards and 3 community cards
        for hole_combo in combinations(hole_cards, 2):
            for comm_combo in combinations(community_cards, 3):
                cards = list(hole_combo) + list(comm_combo)
                result = self._evaluate_five(cards)
                if result[1] > best[1]:
                    best = result

        return best


class FiveCardDrawRules(GameRules):
    """Rules for Five Card Draw poker."""

    @property
    def variant(self) -> int:
        return poker_types.FIVE_CARD_DRAW

    @property
    def hole_card_count(self) -> int:
        return 5

    @property
    def phases(self) -> list:
        return [
            poker_types.PREFLOP,  # Initial betting
            poker_types.DRAW,  # Draw phase
            poker_types.SHOWDOWN,
        ]

    def get_next_phase(self, current_phase: int) -> Optional[PhaseTransition]:
        if current_phase == poker_types.PREFLOP:
            return PhaseTransition(
                next_phase=poker_types.DRAW,
                community_cards_to_deal=0,
                is_showdown=False,
            )
        elif current_phase == poker_types.DRAW:
            return PhaseTransition(
                next_phase=poker_types.SHOWDOWN,
                community_cards_to_deal=0,
                is_showdown=True,
            )
        return None

    def evaluate_hand(self, hole_cards: list, community_cards: list) -> tuple:
        """Evaluate 5-card hand (no community cards in draw)."""
        # Reuse Hold'em evaluation since it's standard poker hand ranking
        evaluator = TexasHoldemRules()
        return evaluator._evaluate_five(hole_cards)

    def execute_draw(
        self, deck: list, hole_cards: list, discard_indices: list
    ) -> DrawResult:
        """
        Execute a draw - discard selected cards and draw replacements.

        Args:
            deck: Current deck to draw from
            hole_cards: Player's current hole cards
            discard_indices: Indices of cards to discard (0-4)

        Returns:
            DrawResult with new hand, cards drawn, and remaining deck
        """
        new_hole = hole_cards.copy()
        remaining_deck = deck.copy()

        # Remove discarded cards (in reverse order to maintain indices)
        for i in sorted(discard_indices, reverse=True):
            new_hole.pop(i)

        # Draw new cards
        cards_drawn = []
        for _ in range(len(discard_indices)):
            if remaining_deck:
                card = remaining_deck.pop()
                cards_drawn.append(card)
                new_hole.append(card)

        return DrawResult(
            new_hole_cards=new_hole,
            cards_drawn=cards_drawn,
            remaining_deck=remaining_deck,
        )


def get_game_rules(variant: int) -> GameRules:
    """Factory function to get rules for a game variant."""
    if variant == poker_types.TEXAS_HOLDEM:
        return TexasHoldemRules()
    elif variant == poker_types.OMAHA:
        return OmahaRules()
    elif variant == poker_types.FIVE_CARD_DRAW:
        return FiveCardDrawRules()
    else:
        # Default to Hold'em
        return TexasHoldemRules()
