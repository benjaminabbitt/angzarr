"""Tests for Hand aggregate."""

import pytest

from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import types_pb2 as poker_types

from .hand import Hand


def make_players(count=2, stack=1000):
    """Helper to create player list for dealing."""
    return [
        hand.PlayerInHand(
            player_root=bytes([i + 1] * 4),
            position=i,
            stack=stack,
        )
        for i in range(count)
    ]


class TestDeal:
    """Test Hand.deal()."""

    def test_deal_creates_hand(self):
        h = Hand()
        assert not h.exists

        players = make_players(2)
        cmd = hand.DealCards(
            table_root=b"\xaa\xbb\xcc\xdd",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        )
        event = h.deal(cmd)

        assert h.exists
        assert h.status == "betting"
        assert h.hand_number == 1
        assert len(h.players) == 2
        assert event.hand_number == 1

    def test_deal_assigns_hole_cards(self):
        h = Hand()
        players = make_players(2)
        cmd = hand.DealCards(
            table_root=b"\xaa\xbb\xcc\xdd",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        )
        h.deal(cmd)

        for player in h.players.values():
            assert len(player.hole_cards) == 2

    def test_deal_rejects_existing_hand(self):
        h = Hand()
        players = make_players(2)
        cmd = hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        )
        h.deal(cmd)

        with pytest.raises(CommandRejectedError, match="already dealt"):
            h.deal(cmd)

    def test_deal_requires_two_players(self):
        h = Hand()
        cmd = hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=make_players(1),
        )

        with pytest.raises(CommandRejectedError, match="at least 2"):
            h.deal(cmd)


class TestPostBlind:
    """Test Hand.post_blind()."""

    def test_post_blind_updates_pot(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))

        event = h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))

        assert event.pot_total == 5
        assert event.player_stack == 995

    def test_post_blinds_sequence(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))

        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        event = h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))

        assert event.pot_total == 15
        assert h.current_bet == 10


class TestAction:
    """Test Hand.action()."""

    def _setup_hand(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        return h, players

    def test_call_action(self):
        h, players = self._setup_hand()

        event = h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,  # Call the remaining 5
        ))

        assert event.action == poker_types.CALL
        assert event.amount == 5

    def test_fold_action(self):
        h, players = self._setup_hand()

        event = h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))

        assert event.action == poker_types.FOLD
        player = h.get_player(players[0].player_root)
        assert player.has_folded

    def test_raise_action(self):
        h, players = self._setup_hand()

        event = h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.RAISE,
            amount=20,
        ))

        assert event.action == poker_types.RAISE

    def test_check_rejects_when_bet_exists(self):
        h, players = self._setup_hand()

        with pytest.raises(CommandRejectedError, match="Cannot check"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.CHECK,
            ))

    def test_action_rejects_folded_player(self):
        h, players = self._setup_hand()
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))

        with pytest.raises(CommandRejectedError, match="has folded"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.CHECK,
            ))


class TestDealCommunity:
    """Test Hand.deal_community()."""

    def _setup_hand_with_betting(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        # Complete preflop betting
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.CHECK,
        ))
        return h, players

    def test_deal_flop(self):
        h, players = self._setup_hand_with_betting()

        event = h.deal_community(hand.DealCommunityCards(count=3))

        assert len(event.cards) == 3
        assert h.current_phase == poker_types.FLOP
        assert len(h.community_cards) == 3

    def test_deal_turn(self):
        h, players = self._setup_hand_with_betting()
        h.deal_community(hand.DealCommunityCards(count=3))  # Flop

        event = h.deal_community(hand.DealCommunityCards(count=1))

        assert len(event.cards) == 1
        assert h.current_phase == poker_types.TURN
        assert len(h.community_cards) == 4

    def test_deal_river(self):
        h, players = self._setup_hand_with_betting()
        h.deal_community(hand.DealCommunityCards(count=3))  # Flop
        h.deal_community(hand.DealCommunityCards(count=1))  # Turn

        event = h.deal_community(hand.DealCommunityCards(count=1))

        assert len(event.cards) == 1
        assert h.current_phase == poker_types.RIVER
        assert len(h.community_cards) == 5


class TestAward:
    """Test Hand.award()."""

    def test_award_completes_hand(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        # Player 0 folds
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))

        pot_event, complete_event = h.award(hand.AwardPot(
            awards=[hand.PotAward(
                player_root=players[1].player_root,
                amount=15,
                pot_type="main",
            )],
        ))

        assert h.status == "complete"
        assert len(pot_event.winners) == 1
        assert pot_event.winners[0].amount == 15

    def test_award_rejects_folded_winner(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))

        with pytest.raises(CommandRejectedError, match="Folded player"):
            h.award(hand.AwardPot(
                awards=[hand.PotAward(
                    player_root=players[0].player_root,
                    amount=15,
                    pot_type="main",
                )],
            ))


class TestEventBook:
    """Test event book tracking."""

    def test_event_book_has_all_events(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))

        eb = h.event_book()
        assert len(eb.pages) == 3  # deal + 2 blinds

    def test_award_adds_two_events(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))
        h.award(hand.AwardPot(
            awards=[hand.PotAward(
                player_root=players[1].player_root,
                amount=15,
                pot_type="main",
            )],
        ))

        eb = h.event_book()
        # deal + 2 blinds + fold + PotAwarded + HandComplete = 6
        assert len(eb.pages) == 6


class TestStateAccessors:
    """Test state accessor properties."""

    def test_hand_id_format(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa\xbb\xcc\xdd",
            hand_number=5,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        assert h.hand_id == "aabbccdd_5"

    def test_table_root_accessor(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\x01\x02\x03\x04",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        assert h.table_root == b"\x01\x02\x03\x04"

    def test_game_variant_accessor(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        assert h.game_variant == poker_types.GameVariant.TEXAS_HOLDEM

    def test_min_raise_accessor(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        assert h.min_raise == 10

    def test_small_blind_accessor(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        assert h.small_blind == 5

    def test_big_blind_accessor(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        assert h.big_blind == 10

    def test_remaining_deck_accessor(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        # 52 cards - 4 hole cards = 48 remaining
        assert len(h.remaining_deck) == 48

    def test_get_pot_total(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        assert h.get_pot_total() == 15

    def test_get_player_found(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        player = h.get_player(players[0].player_root)
        assert player is not None
        assert player.stack == 1000

    def test_get_player_not_found(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        assert h.get_player(b"\x99\x99\x99\x99") is None

    def test_get_active_players(self):
        h = Hand()
        players = make_players(3)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        # All 3 players active
        assert len(h.get_active_players()) == 3

        # One folds
        h.action(hand.PlayerAction(
            player_root=players[2].player_root,
            action=poker_types.FOLD,
        ))
        assert len(h.get_active_players()) == 2

    def test_get_players_in_hand(self):
        h = Hand()
        players = make_players(3)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        assert len(h.get_players_in_hand()) == 3

        h.action(hand.PlayerAction(
            player_root=players[2].player_root,
            action=poker_types.FOLD,
        ))
        assert len(h.get_players_in_hand()) == 2


class TestPostBlindEdgeCases:
    """Test post_blind edge cases."""

    def test_post_blind_requires_hand_dealt(self):
        h = Hand()
        with pytest.raises(CommandRejectedError, match="Hand not dealt"):
            h.post_blind(hand.PostBlind(
                player_root=b"\x01",
                blind_type="small",
                amount=5,
            ))

    def test_post_blind_requires_player_root(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        with pytest.raises(CommandRejectedError, match="player_root"):
            h.post_blind(hand.PostBlind(blind_type="small", amount=5))

    def test_post_blind_requires_player_in_hand(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        with pytest.raises(CommandRejectedError, match="not in hand"):
            h.post_blind(hand.PostBlind(
                player_root=b"\x99\x99\x99\x99",
                blind_type="small",
                amount=5,
            ))

    def test_post_blind_requires_positive_amount(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        with pytest.raises(CommandRejectedError, match="positive"):
            h.post_blind(hand.PostBlind(
                player_root=players[0].player_root,
                blind_type="small",
                amount=0,
            ))


class TestActionEdgeCases:
    """Test action edge cases."""

    def _setup_hand(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        return h, players

    def test_action_requires_hand_dealt(self):
        h = Hand()
        with pytest.raises(CommandRejectedError, match="Hand not dealt"):
            h.action(hand.PlayerAction(
                player_root=b"\x01",
                action=poker_types.FOLD,
            ))

    def test_action_requires_player_root(self):
        h, players = self._setup_hand()
        with pytest.raises(CommandRejectedError, match="player_root"):
            h.action(hand.PlayerAction(action=poker_types.FOLD))

    def test_action_requires_player_in_hand(self):
        h, players = self._setup_hand()
        with pytest.raises(CommandRejectedError, match="not in hand"):
            h.action(hand.PlayerAction(
                player_root=b"\x99\x99\x99\x99",
                action=poker_types.FOLD,
            ))

    def test_call_nothing_to_call(self):
        h, players = self._setup_hand()
        # After small blind calls, big blind can check (nothing to call)
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        with pytest.raises(CommandRejectedError, match="Nothing to call"):
            h.action(hand.PlayerAction(
                player_root=players[1].player_root,
                action=poker_types.CALL,
            ))

    def test_check_allowed_when_no_bet(self):
        h, players = self._setup_hand()
        # SB calls, BB can check
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        event = h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.CHECK,
        ))
        assert event.action == poker_types.CHECK

    def test_bet_when_bet_exists(self):
        h, players = self._setup_hand()
        # There's already a bet (big blind)
        with pytest.raises(CommandRejectedError, match="Cannot bet when there is already a bet"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.BET,
                amount=20,
            ))

    def test_bet_minimum_enforced(self):
        h, players = self._setup_hand()
        # SB calls, BB checks, then SB can bet
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.CHECK,
        ))
        # Deal flop
        h.deal_community(hand.DealCommunityCards(count=3))
        # Now try to bet less than big blind
        with pytest.raises(CommandRejectedError, match="at least"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.BET,
                amount=5,  # Less than BB of 10
            ))

    def test_bet_exceeds_stack(self):
        h, players = self._setup_hand()
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.CHECK,
        ))
        h.deal_community(hand.DealCommunityCards(count=3))
        with pytest.raises(CommandRejectedError, match="exceeds stack"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.BET,
                amount=5000,  # More than 1000 stack
            ))

    def test_raise_when_no_bet(self):
        h, players = self._setup_hand()
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.CHECK,
        ))
        h.deal_community(hand.DealCommunityCards(count=3))
        with pytest.raises(CommandRejectedError, match="Cannot raise when there is no bet"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.RAISE,
                amount=20,
            ))

    def test_raise_exceeds_stack(self):
        h, players = self._setup_hand()
        with pytest.raises(CommandRejectedError, match="exceeds stack"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.RAISE,
                amount=5000,
            ))

    def test_raise_minimum_enforced(self):
        h, players = self._setup_hand()
        # Min raise is big blind (10)
        with pytest.raises(CommandRejectedError, match="at least"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.RAISE,
                amount=7,  # Total bet would be 5+7=12, raise only 2, min is 10
            ))

    def test_all_in_action(self):
        h, players = self._setup_hand()
        event = h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.ALL_IN,
        ))
        assert event.action == poker_types.ALL_IN
        assert event.amount == 995  # 1000 - 5 (small blind)

    def test_all_in_player_cannot_act(self):
        h, players = self._setup_hand()
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.ALL_IN,
        ))
        with pytest.raises(CommandRejectedError, match="all-in"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.CHECK,
            ))

    def test_invalid_action(self):
        h, players = self._setup_hand()
        with pytest.raises(CommandRejectedError, match="Invalid action"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=999,  # Invalid action type
            ))


class TestDealCommunityEdgeCases:
    """Test deal_community edge cases."""

    def test_deal_community_requires_hand(self):
        h = Hand()
        with pytest.raises(CommandRejectedError, match="Hand not dealt"):
            h.deal_community(hand.DealCommunityCards(count=3))

    def test_deal_community_wrong_count(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.CHECK,
        ))
        # Flop should be 3 cards, not 1
        with pytest.raises(CommandRejectedError, match="Expected"):
            h.deal_community(hand.DealCommunityCards(count=1))

    def test_deal_community_zero_cards(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        with pytest.raises(CommandRejectedError, match="at least 1"):
            h.deal_community(hand.DealCommunityCards(count=0))


class TestReveal:
    """Test reveal/showdown."""

    def _setup_showdown(self):
        """Setup hand at showdown."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()

        # CardsDealt event
        players = make_players(2)
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        # Give player 0 a pair of aces
        pc0 = hand.PlayerHoleCards(player_root=players[0].player_root)
        pc0.cards.append(poker_types.Card(suit=poker_types.SPADES, rank=14))
        pc0.cards.append(poker_types.Card(suit=poker_types.HEARTS, rank=14))
        dealt.player_cards.append(pc0)
        # Give player 1 a pair of kings
        pc1 = hand.PlayerHoleCards(player_root=players[1].player_root)
        pc1.cards.append(poker_types.Card(suit=poker_types.SPADES, rank=13))
        pc1.cards.append(poker_types.Card(suit=poker_types.HEARTS, rank=13))
        dealt.player_cards.append(pc1)

        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # ShowdownStarted event
        showdown_any = Any()
        showdown = hand.ShowdownStarted()
        showdown_any.Pack(showdown, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=showdown_any))

        h = Hand(event_book)
        return h, players

    def test_reveal_cards(self):
        h, players = self._setup_showdown()
        assert h.status == "showdown"

        event = h.reveal(hand.RevealCards(
            player_root=players[0].player_root,
            muck=False,
        ))

        assert isinstance(event, hand.CardsRevealed)
        assert len(event.cards) == 2
        assert event.ranking is not None

    def test_muck_cards(self):
        h, players = self._setup_showdown()

        event = h.reveal(hand.RevealCards(
            player_root=players[1].player_root,
            muck=True,
        ))

        assert isinstance(event, hand.CardsMucked)
        assert event.player_root == players[1].player_root

    def test_reveal_requires_showdown(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        with pytest.raises(CommandRejectedError, match="Not in showdown"):
            h.reveal(hand.RevealCards(
                player_root=players[0].player_root,
            ))

    def test_reveal_requires_player_root(self):
        h, players = self._setup_showdown()
        with pytest.raises(CommandRejectedError, match="player_root"):
            h.reveal(hand.RevealCards())

    def test_reveal_requires_player_in_hand(self):
        h, players = self._setup_showdown()
        with pytest.raises(CommandRejectedError, match="not in hand"):
            h.reveal(hand.RevealCards(
                player_root=b"\x99\x99\x99\x99",
            ))


class TestAwardEdgeCases:
    """Test award edge cases."""

    def test_award_requires_hand(self):
        h = Hand()
        with pytest.raises(CommandRejectedError, match="Hand not dealt"):
            h.award(hand.AwardPot(
                awards=[hand.PotAward(
                    player_root=b"\x01",
                    amount=100,
                    pot_type="main",
                )],
            ))

    def test_award_requires_awards(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))
        with pytest.raises(CommandRejectedError, match="No awards"):
            h.award(hand.AwardPot(awards=[]))

    def test_award_requires_winner_in_hand(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))
        with pytest.raises(CommandRejectedError, match="not in hand"):
            h.award(hand.AwardPot(
                awards=[hand.PotAward(
                    player_root=b"\x99\x99\x99\x99",
                    amount=15,
                    pot_type="main",
                )],
            ))

    def test_award_cannot_double_complete(self):
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))
        h.award(hand.AwardPot(
            awards=[hand.PotAward(
                player_root=players[1].player_root,
                amount=15,
                pot_type="main",
            )],
        ))
        with pytest.raises(CommandRejectedError, match="already complete"):
            h.award(hand.AwardPot(
                awards=[hand.PotAward(
                    player_root=players[1].player_root,
                    amount=15,
                    pot_type="main",
                )],
            ))


class TestEventHandlers:
    """Test event handlers for saga-generated events."""

    def test_pot_awarded_updates_stacks(self):
        """PotAwarded event from saga updates player stacks."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()

        # CardsDealt
        players = make_players(2)
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # BlindPosted
        blind = hand.BlindPosted(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
            player_stack=995,
            pot_total=5,
        )
        blind_any = Any()
        blind_any.Pack(blind, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=blind_any))

        # PotAwarded
        pot = hand.PotAwarded()
        pot.winners.append(hand.PotWinner(
            player_root=players[0].player_root,
            amount=100,
            pot_type="main",
        ))
        pot_any = Any()
        pot_any.Pack(pot, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=pot_any))

        h = Hand(event_book)
        player = h.get_player(players[0].player_root)
        # 995 + 100 = 1095
        assert player.stack == 1095

    def test_hand_complete_sets_status(self):
        """HandComplete event sets status to complete."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()

        # CardsDealt
        players = make_players(2)
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # HandComplete
        complete = hand.HandComplete(
            table_root=b"\xaa",
            hand_number=1,
        )
        complete_any = Any()
        complete_any.Pack(complete, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=complete_any))

        h = Hand(event_book)
        assert h.status == "complete"


class TestDealEdgeCases:
    """Test deal edge cases."""

    def test_deal_requires_players(self):
        h = Hand()
        with pytest.raises(CommandRejectedError, match="No players"):
            h.deal(hand.DealCards(
                table_root=b"\xaa",
                hand_number=1,
                game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
                dealer_position=0,
                players=[],
            ))

    def test_deal_with_seed(self):
        """Deal with explicit deck seed for reproducibility."""
        h1 = Hand()
        h2 = Hand()
        players = make_players(2)

        h1.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
            deck_seed=b"seed123",
        ))
        h2.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
            deck_seed=b"seed123",
        ))

        # Same seed should produce same cards
        for pos in h1.players:
            assert h1.players[pos].hole_cards == h2.players[pos].hole_cards


class TestMoreEdgeCases:
    """Additional edge cases for higher coverage."""

    def test_post_blind_on_complete_hand(self):
        """Cannot post blind after hand is complete."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()
        players = make_players(2)

        # CardsDealt
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # HandComplete
        complete = hand.HandComplete(table_root=b"\xaa", hand_number=1)
        complete_any = Any()
        complete_any.Pack(complete, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=complete_any))

        h = Hand(event_book)
        with pytest.raises(CommandRejectedError, match="complete"):
            h.post_blind(hand.PostBlind(
                player_root=players[0].player_root,
                blind_type="small",
                amount=5,
            ))

    def test_post_blind_by_folded_player(self):
        """Folded player cannot post blind."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()
        players = make_players(2)

        # CardsDealt
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # ActionTaken (fold)
        action = hand.ActionTaken(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
            amount=0,
            player_stack=1000,
            pot_total=0,
        )
        action_any = Any()
        action_any.Pack(action, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=action_any))

        h = Hand(event_book)
        with pytest.raises(CommandRejectedError, match="folded"):
            h.post_blind(hand.PostBlind(
                player_root=players[0].player_root,
                blind_type="small",
                amount=5,
            ))

    def test_action_not_in_betting_phase(self):
        """Cannot take action outside betting phase."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()
        players = make_players(2)

        # CardsDealt
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # ShowdownStarted
        showdown = hand.ShowdownStarted()
        showdown_any = Any()
        showdown_any.Pack(showdown, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=showdown_any))

        h = Hand(event_book)
        with pytest.raises(CommandRejectedError, match="Not in betting"):
            h.action(hand.PlayerAction(
                player_root=players[0].player_root,
                action=poker_types.FOLD,
            ))

    def test_bet_converts_to_all_in(self):
        """Bet for entire stack becomes ALL_IN."""
        h = Hand()
        players = make_players(2, stack=100)  # Small stack
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
            amount=5,
        ))
        h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.CHECK,
        ))
        h.deal_community(hand.DealCommunityCards(count=3))

        # Bet entire remaining stack
        event = h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.BET,
            amount=90,  # Entire remaining stack
        ))
        assert event.action == poker_types.ALL_IN

    def test_call_converts_to_all_in(self):
        """Call that uses entire stack becomes ALL_IN."""
        h = Hand()
        players = [
            hand.PlayerInHand(player_root=b"\x01\x01\x01\x01", position=0, stack=20),
            hand.PlayerInHand(player_root=b"\x02\x02\x02\x02", position=1, stack=1000),
        ]
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        # Player 1 raises big
        h.action(hand.PlayerAction(
            player_root=players[1].player_root,
            action=poker_types.RAISE,
            amount=500,
        ))
        # Player 0 calls but can only afford 15 more (20 total - 5 SB = 15 left)
        event = h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.CALL,
        ))
        assert event.action == poker_types.ALL_IN

    def test_deal_community_on_complete_hand(self):
        """Cannot deal community cards after hand complete."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()
        players = make_players(2)

        # CardsDealt
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # HandComplete
        complete = hand.HandComplete(table_root=b"\xaa", hand_number=1)
        complete_any = Any()
        complete_any.Pack(complete, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=complete_any))

        h = Hand(event_book)
        with pytest.raises(CommandRejectedError, match="complete"):
            h.deal_community(hand.DealCommunityCards(count=3))

    def test_reveal_by_folded_player(self):
        """Folded player cannot reveal cards."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        event_book = types.EventBook()
        players = make_players(2)

        # CardsDealt
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # ActionTaken (fold)
        action = hand.ActionTaken(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
            amount=0,
            player_stack=1000,
            pot_total=0,
        )
        action_any = Any()
        action_any.Pack(action, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=action_any))

        # ShowdownStarted
        showdown = hand.ShowdownStarted()
        showdown_any = Any()
        showdown_any.Pack(showdown, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=showdown_any))

        h = Hand(event_book)
        with pytest.raises(CommandRejectedError, match="folded"):
            h.reveal(hand.RevealCards(
                player_root=players[0].player_root,
            ))

    def test_reveal_requires_hand_dealt(self):
        """Cannot reveal without a hand."""
        h = Hand()
        with pytest.raises(CommandRejectedError, match="Hand not dealt"):
            h.reveal(hand.RevealCards(player_root=b"\x01"))

    def test_raise_converts_to_all_in(self):
        """Raise for entire stack becomes ALL_IN."""
        h = Hand()
        players = make_players(2, stack=100)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        # Raise for entire remaining stack (95)
        event = h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.RAISE,
            amount=95,
        ))
        assert event.action == poker_types.ALL_IN

    def test_five_card_draw_no_community_cards(self):
        """Five card draw variant doesn't have community cards."""
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.FIVE_CARD_DRAW,
            dealer_position=0,
            players=players,
        ))
        with pytest.raises(CommandRejectedError, match="Five card draw"):
            h.deal_community(hand.DealCommunityCards(count=3))

    def test_no_more_phases_after_showdown(self):
        """Cannot deal community cards in showdown phase."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        # Build a hand already in showdown
        event_book = types.EventBook()
        players = make_players(2)

        # CardsDealt event
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        # ShowdownStarted event - puts us in showdown phase
        showdown = hand.ShowdownStarted()
        showdown_any = Any()
        showdown_any.Pack(showdown, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=showdown_any))

        h = Hand(event_book)
        # Current phase is showdown, get_next_phase returns None
        # Force current_phase to SHOWDOWN
        h._get_state().current_phase = poker_types.SHOWDOWN

        with pytest.raises(CommandRejectedError, match="No more phases"):
            h.deal_community(hand.DealCommunityCards(count=1))

    def test_not_enough_cards_in_deck(self):
        """Cannot deal if deck is exhausted."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        # Build a hand with minimal remaining deck
        event_book = types.EventBook()
        players = make_players(2)

        # CardsDealt event
        dealt = hand.CardsDealt(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
        )
        dealt.players.extend(players)
        dealt_any = Any()
        dealt_any.Pack(dealt, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=dealt_any))

        h = Hand(event_book)
        # Manually set remaining_deck to be too small
        h._get_state().remaining_deck = [(poker_types.SPADES, 2)]

        with pytest.raises(CommandRejectedError, match="Not enough cards"):
            h.deal_community(hand.DealCommunityCards(count=3))

    def test_award_adjusts_pot_total(self):
        """Award adjusts first winner's amount to match pot."""
        h = Hand()
        players = make_players(2)
        h.deal(hand.DealCards(
            table_root=b"\xaa",
            hand_number=1,
            game_variant=poker_types.GameVariant.TEXAS_HOLDEM,
            dealer_position=0,
            players=players,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[0].player_root,
            blind_type="small",
            amount=5,
        ))
        h.post_blind(hand.PostBlind(
            player_root=players[1].player_root,
            blind_type="big",
            amount=10,
        ))
        h.action(hand.PlayerAction(
            player_root=players[0].player_root,
            action=poker_types.FOLD,
        ))

        # Award with wrong amount - should be adjusted to pot total (15)
        pot_event, complete_event = h.award(hand.AwardPot(
            awards=[hand.PotAward(
                player_root=players[1].player_root,
                amount=10,  # Wrong amount, pot is 15
                pot_type="main",
            )],
        ))

        # First winner's amount should be adjusted to pot total
        assert pot_event.winners[0].amount == 15
