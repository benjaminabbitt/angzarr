"""Tests for game_rules.py - poker variant rules."""

import pytest
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from .game_rules import (
    DealResult,
    DrawResult,
    FiveCardDrawRules,
    GameRules,
    OmahaRules,
    PhaseTransition,
    TexasHoldemRules,
    get_game_rules,
)


# =============================================================================
# OmahaRules tests
# =============================================================================


class TestOmahaRulesProperties:
    def test_variant_returns_omaha(self):
        rules = OmahaRules()
        assert rules.variant == poker_types.OMAHA

    def test_hole_card_count_returns_four(self):
        rules = OmahaRules()
        assert rules.hole_card_count == 4

    def test_phases_same_as_holdem(self):
        rules = OmahaRules()
        assert rules.phases == [
            poker_types.PREFLOP,
            poker_types.FLOP,
            poker_types.TURN,
            poker_types.RIVER,
            poker_types.SHOWDOWN,
        ]


class TestOmahaRulesEvaluateHand:
    def test_evaluate_hand_uses_two_hole_three_community(self):
        rules = OmahaRules()
        # Hole cards: AA KK (4 cards)
        hole = [
            (poker_types.SPADES, 14),
            (poker_types.HEARTS, 14),
            (poker_types.SPADES, 13),
            (poker_types.HEARTS, 13),
        ]
        # Community: QQQ (3 queens)
        community = [
            (poker_types.SPADES, 12),
            (poker_types.HEARTS, 12),
            (poker_types.DIAMONDS, 12),
            (poker_types.CLUBS, 10),
            (poker_types.CLUBS, 9),
        ]
        rank_type, score, kickers = rules.evaluate_hand(hole, community)
        # Best hand: AAA QQ (full house) using 2 aces + 2 queens + 1 ace from community?
        # Actually: must use exactly 2 hole, 3 community
        # Best: AA (2 hole) + QQQ (3 community) = AAQ QQ full house? No wait...
        # Using AA from hole + Q Q Q from community = AAQQ + Q = full house QQQ AA
        assert rank_type == poker_types.FULL_HOUSE

    def test_evaluate_hand_straight_flush(self):
        rules = OmahaRules()
        # Hole: 10s Js and garbage
        hole = [
            (poker_types.SPADES, 10),
            (poker_types.SPADES, 11),
            (poker_types.HEARTS, 2),
            (poker_types.HEARTS, 3),
        ]
        # Community: 8s 9s Qs + garbage
        community = [
            (poker_types.SPADES, 8),
            (poker_types.SPADES, 9),
            (poker_types.SPADES, 12),
            (poker_types.HEARTS, 4),
            (poker_types.DIAMONDS, 5),
        ]
        rank_type, score, _ = rules.evaluate_hand(hole, community)
        # Best: 10s Js (hole) + 8s 9s Qs (community) = 8-9-10-J-Q straight flush
        assert rank_type == poker_types.STRAIGHT_FLUSH

    def test_evaluate_hand_high_card_only(self):
        rules = OmahaRules()
        # All mismatched cards
        hole = [
            (poker_types.SPADES, 2),
            (poker_types.HEARTS, 4),
            (poker_types.DIAMONDS, 6),
            (poker_types.CLUBS, 8),
        ]
        community = [
            (poker_types.SPADES, 14),
            (poker_types.HEARTS, 13),
            (poker_types.DIAMONDS, 11),
            (poker_types.CLUBS, 9),
            (poker_types.SPADES, 7),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, community)
        # Using 8c 6d from hole + A K J from community = high card A
        assert rank_type == poker_types.HIGH_CARD


# =============================================================================
# FiveCardDrawRules tests
# =============================================================================


class TestFiveCardDrawRulesProperties:
    def test_variant_returns_five_card_draw(self):
        rules = FiveCardDrawRules()
        assert rules.variant == poker_types.FIVE_CARD_DRAW

    def test_hole_card_count_returns_five(self):
        rules = FiveCardDrawRules()
        assert rules.hole_card_count == 5

    def test_phases_includes_draw(self):
        rules = FiveCardDrawRules()
        assert rules.phases == [
            poker_types.PREFLOP,
            poker_types.DRAW,
            poker_types.SHOWDOWN,
        ]


class TestFiveCardDrawRulesGetNextPhase:
    def test_preflop_to_draw(self):
        rules = FiveCardDrawRules()
        result = rules.get_next_phase(poker_types.PREFLOP)
        assert result is not None
        assert result.next_phase == poker_types.DRAW
        assert result.community_cards_to_deal == 0
        assert result.is_showdown is False

    def test_draw_to_showdown(self):
        rules = FiveCardDrawRules()
        result = rules.get_next_phase(poker_types.DRAW)
        assert result is not None
        assert result.next_phase == poker_types.SHOWDOWN
        assert result.community_cards_to_deal == 0
        assert result.is_showdown is True

    def test_showdown_returns_none(self):
        rules = FiveCardDrawRules()
        result = rules.get_next_phase(poker_types.SHOWDOWN)
        assert result is None


class TestFiveCardDrawRulesEvaluateHand:
    def test_evaluate_hand_pair(self):
        rules = FiveCardDrawRules()
        hole = [
            (poker_types.SPADES, 10),
            (poker_types.HEARTS, 10),
            (poker_types.DIAMONDS, 8),
            (poker_types.CLUBS, 6),
            (poker_types.SPADES, 4),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, [])
        assert rank_type == poker_types.PAIR

    def test_evaluate_hand_full_house(self):
        rules = FiveCardDrawRules()
        hole = [
            (poker_types.SPADES, 10),
            (poker_types.HEARTS, 10),
            (poker_types.DIAMONDS, 10),
            (poker_types.CLUBS, 8),
            (poker_types.SPADES, 8),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, [])
        assert rank_type == poker_types.FULL_HOUSE

    def test_evaluate_hand_ignores_community(self):
        rules = FiveCardDrawRules()
        hole = [
            (poker_types.SPADES, 14),
            (poker_types.HEARTS, 13),
            (poker_types.DIAMONDS, 12),
            (poker_types.CLUBS, 11),
            (poker_types.SPADES, 10),
        ]
        # Pass community cards - should be ignored
        community = [
            (poker_types.HEARTS, 14),
            (poker_types.HEARTS, 14),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.STRAIGHT


class TestFiveCardDrawRulesExecuteDraw:
    def test_execute_draw_replaces_cards(self):
        rules = FiveCardDrawRules()
        deck = [
            (poker_types.SPADES, 14),
            (poker_types.HEARTS, 13),
            (poker_types.DIAMONDS, 12),
        ]
        hole = [
            (poker_types.CLUBS, 2),
            (poker_types.CLUBS, 3),
            (poker_types.CLUBS, 4),
            (poker_types.CLUBS, 5),
            (poker_types.CLUBS, 6),
        ]
        # Discard first two cards
        result = rules.execute_draw(deck, hole, [0, 1])

        assert len(result.new_hole_cards) == 5
        assert len(result.cards_drawn) == 2
        assert len(result.remaining_deck) == 1
        # Original cards 4, 5, 6 should still be present
        assert (poker_types.CLUBS, 4) in result.new_hole_cards
        assert (poker_types.CLUBS, 5) in result.new_hole_cards
        assert (poker_types.CLUBS, 6) in result.new_hole_cards

    def test_execute_draw_no_discard(self):
        rules = FiveCardDrawRules()
        deck = [(poker_types.SPADES, 14)]
        hole = [
            (poker_types.CLUBS, 10),
            (poker_types.CLUBS, 11),
            (poker_types.CLUBS, 12),
            (poker_types.CLUBS, 13),
            (poker_types.CLUBS, 14),
        ]
        result = rules.execute_draw(deck, hole, [])

        assert result.new_hole_cards == hole
        assert result.cards_drawn == []
        assert len(result.remaining_deck) == 1

    def test_execute_draw_all_cards(self):
        rules = FiveCardDrawRules()
        deck = [
            (poker_types.SPADES, 10),
            (poker_types.SPADES, 11),
            (poker_types.SPADES, 12),
            (poker_types.SPADES, 13),
            (poker_types.SPADES, 14),
        ]
        hole = [
            (poker_types.CLUBS, 2),
            (poker_types.CLUBS, 3),
            (poker_types.CLUBS, 4),
            (poker_types.CLUBS, 5),
            (poker_types.CLUBS, 6),
        ]
        result = rules.execute_draw(deck, hole, [0, 1, 2, 3, 4])

        assert len(result.new_hole_cards) == 5
        assert len(result.cards_drawn) == 5
        assert len(result.remaining_deck) == 0
        # All original cards should be replaced
        assert (poker_types.CLUBS, 2) not in result.new_hole_cards

    def test_execute_draw_partial_deck(self):
        rules = FiveCardDrawRules()
        deck = [(poker_types.SPADES, 14)]  # Only one card
        hole = [
            (poker_types.CLUBS, 2),
            (poker_types.CLUBS, 3),
            (poker_types.CLUBS, 4),
            (poker_types.CLUBS, 5),
            (poker_types.CLUBS, 6),
        ]
        # Try to discard 3 but only 1 card in deck
        result = rules.execute_draw(deck, hole, [0, 1, 2])

        assert len(result.new_hole_cards) == 3  # 5 - 3 discarded + 1 drawn
        assert len(result.cards_drawn) == 1
        assert len(result.remaining_deck) == 0


# =============================================================================
# get_game_rules factory tests
# =============================================================================


class TestGetGameRulesFactory:
    def test_returns_texas_holdem(self):
        rules = get_game_rules(poker_types.TEXAS_HOLDEM)
        assert isinstance(rules, TexasHoldemRules)
        assert rules.variant == poker_types.TEXAS_HOLDEM

    def test_returns_omaha(self):
        rules = get_game_rules(poker_types.OMAHA)
        assert isinstance(rules, OmahaRules)
        assert rules.variant == poker_types.OMAHA

    def test_returns_five_card_draw(self):
        rules = get_game_rules(poker_types.FIVE_CARD_DRAW)
        assert isinstance(rules, FiveCardDrawRules)
        assert rules.variant == poker_types.FIVE_CARD_DRAW

    def test_unknown_variant_defaults_to_holdem(self):
        rules = get_game_rules(9999)  # Unknown variant
        assert isinstance(rules, TexasHoldemRules)


# =============================================================================
# Hand evaluation edge cases (TexasHoldemRules)
# =============================================================================


class TestHandEvaluationEdgeCases:
    def test_wheel_straight(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 14), (poker_types.HEARTS, 2)]
        community = [
            (poker_types.DIAMONDS, 3),
            (poker_types.CLUBS, 4),
            (poker_types.SPADES, 5),
            (poker_types.HEARTS, 9),
            (poker_types.DIAMONDS, 10),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.STRAIGHT

    def test_royal_flush(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 14), (poker_types.SPADES, 13)]
        community = [
            (poker_types.SPADES, 12),
            (poker_types.SPADES, 11),
            (poker_types.SPADES, 10),
            (poker_types.HEARTS, 2),
            (poker_types.DIAMONDS, 3),
        ]
        rank_type, score, _ = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.ROYAL_FLUSH
        assert score == 10000000

    def test_four_of_a_kind(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 10), (poker_types.HEARTS, 10)]
        community = [
            (poker_types.DIAMONDS, 10),
            (poker_types.CLUBS, 10),
            (poker_types.SPADES, 5),
            (poker_types.HEARTS, 6),
            (poker_types.DIAMONDS, 7),
        ]
        rank_type, _, kickers = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.FOUR_OF_A_KIND
        assert len(kickers) == 1  # Has one kicker

    def test_flush(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 2), (poker_types.SPADES, 4)]
        community = [
            (poker_types.SPADES, 6),
            (poker_types.SPADES, 8),
            (poker_types.SPADES, 10),
            (poker_types.HEARTS, 14),
            (poker_types.DIAMONDS, 13),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.FLUSH

    def test_three_of_a_kind(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 10), (poker_types.HEARTS, 10)]
        community = [
            (poker_types.DIAMONDS, 10),
            (poker_types.CLUBS, 5),
            (poker_types.SPADES, 6),
            (poker_types.HEARTS, 7),
            (poker_types.DIAMONDS, 8),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.THREE_OF_A_KIND

    def test_two_pair(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 10), (poker_types.HEARTS, 10)]
        community = [
            (poker_types.DIAMONDS, 8),
            (poker_types.CLUBS, 8),
            (poker_types.SPADES, 5),
            (poker_types.HEARTS, 6),
            (poker_types.DIAMONDS, 7),
        ]
        rank_type, _, kickers = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.TWO_PAIR
        assert len(kickers) == 1  # Has one kicker

    def test_pair(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 10), (poker_types.HEARTS, 10)]
        community = [
            (poker_types.DIAMONDS, 2),
            (poker_types.CLUBS, 4),
            (poker_types.SPADES, 6),
            (poker_types.HEARTS, 8),
            (poker_types.DIAMONDS, 12),
        ]
        rank_type, _, kickers = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.PAIR
        assert len(kickers) == 3  # Pair has 3 kickers

    def test_high_card(self):
        rules = TexasHoldemRules()
        hole = [(poker_types.SPADES, 14), (poker_types.HEARTS, 2)]
        community = [
            (poker_types.DIAMONDS, 4),
            (poker_types.CLUBS, 6),
            (poker_types.SPADES, 8),
            (poker_types.HEARTS, 10),
            (poker_types.DIAMONDS, 12),
        ]
        rank_type, _, _ = rules.evaluate_hand(hole, community)
        assert rank_type == poker_types.HIGH_CARD

    def test_less_than_five_cards_returns_high_card(self):
        rules = TexasHoldemRules()
        # Only 4 cards
        result = rules._find_best_hand([
            (poker_types.SPADES, 14),
            (poker_types.HEARTS, 13),
            (poker_types.DIAMONDS, 12),
            (poker_types.CLUBS, 11),
        ])
        assert result[0] == poker_types.HIGH_CARD
        assert result[1] == 0


# =============================================================================
# Deck creation and dealing tests
# =============================================================================


class TestDeckOperations:
    def test_create_deck_has_52_cards(self):
        rules = TexasHoldemRules()
        deck = rules.create_deck()
        assert len(deck) == 52

    def test_create_deck_with_seed_is_deterministic(self):
        rules = TexasHoldemRules()
        seed = b"test_seed_value"
        deck1 = rules.create_deck(seed)
        deck2 = rules.create_deck(seed)
        assert deck1 == deck2

    def test_deal_hole_cards_omaha(self):
        rules = OmahaRules()
        players = [b"\x01", b"\x02"]
        result = rules.deal_hole_cards([], players, seed=b"seed1234")

        assert len(result.player_cards) == 2
        assert len(result.player_cards[b"\x01"]) == 4
        assert len(result.player_cards[b"\x02"]) == 4
        assert len(result.remaining_deck) == 52 - 8

    def test_deal_hole_cards_five_card_draw(self):
        rules = FiveCardDrawRules()
        players = [b"\x01", b"\x02"]
        result = rules.deal_hole_cards([], players, seed=b"seed1234")

        assert len(result.player_cards) == 2
        assert len(result.player_cards[b"\x01"]) == 5
        assert len(result.player_cards[b"\x02"]) == 5
        assert len(result.remaining_deck) == 52 - 10

    def test_deal_hole_cards_with_existing_deck(self):
        """Test dealing from pre-existing deck (no seed)."""
        rules = TexasHoldemRules()
        # Pre-create a partial deck
        existing_deck = [
            (poker_types.SPADES, 14),
            (poker_types.HEARTS, 14),
            (poker_types.DIAMONDS, 14),
            (poker_types.CLUBS, 14),
            (poker_types.SPADES, 13),
            (poker_types.HEARTS, 13),
        ]
        players = [b"\x01", b"\x02"]
        result = rules.deal_hole_cards(existing_deck, players)

        assert len(result.player_cards) == 2
        assert len(result.player_cards[b"\x01"]) == 2
        assert len(result.player_cards[b"\x02"]) == 2
        assert len(result.remaining_deck) == 2


class TestTexasHoldemPhases:
    def test_preflop_to_flop(self):
        rules = TexasHoldemRules()
        result = rules.get_next_phase(poker_types.PREFLOP)
        assert result.next_phase == poker_types.FLOP
        assert result.community_cards_to_deal == 3

    def test_flop_to_turn(self):
        rules = TexasHoldemRules()
        result = rules.get_next_phase(poker_types.FLOP)
        assert result.next_phase == poker_types.TURN
        assert result.community_cards_to_deal == 1

    def test_turn_to_river(self):
        rules = TexasHoldemRules()
        result = rules.get_next_phase(poker_types.TURN)
        assert result.next_phase == poker_types.RIVER
        assert result.community_cards_to_deal == 1

    def test_river_to_showdown(self):
        rules = TexasHoldemRules()
        result = rules.get_next_phase(poker_types.RIVER)
        assert result.next_phase == poker_types.SHOWDOWN
        assert result.community_cards_to_deal == 0
        assert result.is_showdown is True

    def test_showdown_returns_none(self):
        rules = TexasHoldemRules()
        result = rules.get_next_phase(poker_types.SHOWDOWN)
        assert result is None
