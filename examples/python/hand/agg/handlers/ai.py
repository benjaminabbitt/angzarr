"""AI decision making for poker."""

import random
from collections import Counter
from dataclasses import dataclass
from typing import Tuple, List

from angzarr_client.proto.examples import types_pb2 as poker_types


@dataclass
class BettingDecision:
    """Result of AI betting decision."""

    action: str  # "fold", "check", "call", "bet", "raise"
    amount: int
    reasoning: str


@dataclass
class DrawDecision:
    """Result of AI draw decision."""

    discard_indices: List[int]
    reasoning: str


class PokerAI:
    """AI player for poker games."""

    def __init__(self, rules):
        """
        Initialize AI with game rules.

        Args:
            rules: GameRules instance for hand evaluation
        """
        self.rules = rules

    def decide_action(
        self,
        hole_cards: list,
        community_cards: list,
        to_call: int,
        pot: int,
        current_bet: int,
        stack: int,
        big_blind: int,
        is_holdem_preflop: bool = False,
    ) -> BettingDecision:
        """
        Decide betting action based on hand strength and pot odds.

        Args:
            hole_cards: Player's hole cards
            community_cards: Community cards on board
            to_call: Amount needed to call
            pot: Current pot size
            current_bet: Current bet to match
            stack: Player's remaining stack
            big_blind: Big blind amount
            is_holdem_preflop: True if Hold'em preflop (2 cards, no community)

        Returns:
            BettingDecision with action, amount, and reasoning
        """
        # Evaluate current hand
        if community_cards:
            hand_result = self.rules.evaluate_hand(hole_cards, community_cards)
        else:
            hand_result = self.rules.evaluate_hand(hole_cards, [])

        hand_type = hand_result[0]
        hand_name = self._hand_name(hand_type)

        # For Hold'em preflop with only 2 cards, use simplified logic
        if is_holdem_preflop and len(hole_cards) == 2:
            hand_type, hand_name = self._evaluate_holdem_preflop(hole_cards)

        pot_odds = to_call / (pot + to_call + 1) if to_call > 0 else 0

        if to_call == 0:
            return self._decide_no_bet(hand_type, hand_name, pot, stack, big_blind)
        else:
            return self._decide_facing_bet(
                hand_type, hand_name, to_call, pot, pot_odds,
                current_bet, stack, big_blind
            )

    def _evaluate_holdem_preflop(self, hole_cards: list) -> Tuple[int, str]:
        """Evaluate preflop hand strength for Hold'em."""
        ranks = sorted([c[1] for c in hole_cards], reverse=True)
        suited = hole_cards[0][0] == hole_cards[1][0]
        pair = ranks[0] == ranks[1]
        high = ranks[0] >= 12

        rank_symbols = {
            2: "2", 3: "3", 4: "4", 5: "5", 6: "6", 7: "7", 8: "8", 9: "9",
            10: "T", 11: "J", 12: "Q", 13: "K", 14: "A",
        }

        if pair:
            strength = 3 + ranks[0] / 14
            hand_name = f"Pair of {rank_symbols[ranks[0]]}s"
        elif high and suited:
            strength = 2.5
            hand_name = "High suited"
        elif high:
            strength = 2
            hand_name = "High cards"
        else:
            strength = ranks[0] / 14
            hand_name = "Weak"

        if strength > 2.5:
            hand_type = poker_types.THREE_OF_A_KIND
        elif strength > 1.5:
            hand_type = poker_types.PAIR
        else:
            hand_type = poker_types.HIGH_CARD

        return hand_type, hand_name

    def _decide_no_bet(
        self, hand_type: int, hand_name: str, pot: int, stack: int, big_blind: int
    ) -> BettingDecision:
        """Decide action when no bet to call."""
        if hand_type >= poker_types.TWO_PAIR:
            bet_size = min(pot // 2 + big_blind, stack)
            if bet_size > 0:
                return BettingDecision("bet", bet_size, f"{hand_name} → BET")

        if random.random() < 0.3 and hand_type >= poker_types.PAIR:
            bet_size = min(big_blind * 2, stack)
            return BettingDecision("bet", bet_size, f"{hand_name} → BET")

        return BettingDecision("check", 0, f"{hand_name} → CHECK")

    def _decide_facing_bet(
        self, hand_type: int, hand_name: str, to_call: int, pot: int,
        pot_odds: float, current_bet: int, stack: int, big_blind: int
    ) -> BettingDecision:
        """Decide action when facing a bet."""
        if hand_type >= poker_types.THREE_OF_A_KIND:
            raise_to = min(current_bet * 2 + big_blind, stack + current_bet)
            if raise_to > current_bet and stack > to_call:
                return BettingDecision(
                    "raise", raise_to - current_bet,
                    f"{hand_name} → RAISE"
                )
            return BettingDecision("call", to_call, f"{hand_name} → CALL")

        elif hand_type >= poker_types.PAIR:
            if pot_odds < 0.4 or random.random() < 0.7:
                if stack >= to_call:
                    return BettingDecision(
                        "call", to_call,
                        f"{hand_name}, odds OK → CALL"
                    )
            return BettingDecision("fold", 0, f"{hand_name}, odds bad → FOLD")

        else:
            if pot_odds < 0.15 and random.random() < 0.3:
                return BettingDecision("call", to_call, f"{hand_name}, speculative → CALL")
            return BettingDecision("fold", 0, f"{hand_name} → FOLD")

    def decide_draw(self, hole_cards: list) -> DrawDecision:
        """
        Decide which cards to discard in Five Card Draw.

        Args:
            hole_cards: Player's current 5 hole cards

        Returns:
            DrawDecision with discard indices and reasoning
        """
        ranks = [c[1] for c in hole_cards]
        suits = [c[0] for c in hole_cards]

        rank_counts = Counter(ranks)
        suit_counts = Counter(suits)

        pairs = [r for r, c in rank_counts.items() if c == 2]
        trips = [r for r, c in rank_counts.items() if c == 3]
        quads = [r for r, c in rank_counts.items() if c == 4]

        rank_sym = {
            2: "2", 3: "3", 4: "4", 5: "5", 6: "6", 7: "7", 8: "8", 9: "9",
            10: "T", 11: "J", 12: "Q", 13: "K", 14: "A",
        }
        suit_sym = {
            poker_types.CLUBS: "♣",
            poker_types.DIAMONDS: "♦",
            poker_types.HEARTS: "♥",
            poker_types.SPADES: "♠",
        }

        # Keep quads - discard the one other card
        if quads:
            quad_rank = quads[0]
            discard = [i for i, (s, r) in enumerate(hole_cards) if r != quad_rank]
            return DrawDecision(
                discard,
                f"Four of a Kind ({rank_sym[quad_rank]}s)"
            )

        # Keep trips - discard two others
        if trips:
            trip_rank = trips[0]
            discard = [i for i, (s, r) in enumerate(hole_cards) if r != trip_rank]
            return DrawDecision(
                discard,
                f"Three of a Kind ({rank_sym[trip_rank]}s)"
            )

        # Keep two pair - discard the kicker
        if len(pairs) == 2:
            discard = [i for i, (s, r) in enumerate(hole_cards) if r not in pairs]
            return DrawDecision(
                discard,
                f"Two Pair ({rank_sym[pairs[0]]}s and {rank_sym[pairs[1]]}s)"
            )

        # Keep one pair - discard three others
        if len(pairs) == 1:
            pair_rank = pairs[0]
            discard = [i for i, (s, r) in enumerate(hole_cards) if r != pair_rank]
            return DrawDecision(
                discard,
                f"Pair of {rank_sym[pair_rank]}s"
            )

        # Check for 4-flush (4 cards same suit)
        for suit, count in suit_counts.items():
            if count >= 4:
                discard = [i for i, (s, r) in enumerate(hole_cards) if s != suit]
                return DrawDecision(
                    discard,
                    f"4-card flush draw ({suit_sym[suit]})"
                )

        # Check for 4-straight (4 cards in sequence)
        sorted_ranks = sorted(set(ranks))
        for i in range(len(sorted_ranks) - 3):
            if sorted_ranks[i + 3] - sorted_ranks[i] == 3:
                straight_ranks = set(range(sorted_ranks[i], sorted_ranks[i] + 4))
                discard = [j for j, (s, r) in enumerate(hole_cards)
                           if r not in straight_ranks]
                if len(discard) <= 1:
                    return DrawDecision(discard, "4-card straight draw")

        # No made hand or draw - keep highest 2 cards
        indexed_cards = [(i, hole_cards[i]) for i in range(len(hole_cards))]
        indexed_cards.sort(key=lambda x: x[1][1], reverse=True)

        keep_indices = {indexed_cards[0][0], indexed_cards[1][0]}
        discard = [i for i in range(len(hole_cards)) if i not in keep_indices]

        kept = [hole_cards[i] for i in keep_indices]
        kept_str = ", ".join(f"{rank_sym[r]}{suit_sym[s]}" for s, r in kept)
        return DrawDecision(discard, f"No made hand - keeping {kept_str}")

    def _hand_name(self, hand_type: int) -> str:
        """Get human-readable hand name."""
        names = {
            poker_types.HIGH_CARD: "High Card",
            poker_types.PAIR: "Pair",
            poker_types.TWO_PAIR: "Two Pair",
            poker_types.THREE_OF_A_KIND: "Three of a Kind",
            poker_types.STRAIGHT: "Straight",
            poker_types.FLUSH: "Flush",
            poker_types.FULL_HOUSE: "Full House",
            poker_types.FOUR_OF_A_KIND: "Four of a Kind",
            poker_types.STRAIGHT_FLUSH: "Straight Flush",
            poker_types.ROYAL_FLUSH: "Royal Flush",
        }
        return names.get(hand_type, "Unknown")
