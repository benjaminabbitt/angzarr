"""Tests for Player aggregate functional handlers.

Tests follow the guard/validate/compute pattern, testing pure functions
directly without infrastructure dependencies.
"""

import pytest
from google.protobuf.any_pb2 import Any as AnyProto

from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from .commands import (
    handle_deposit_funds,
    handle_register_player,
    handle_release_funds,
    handle_reserve_funds,
    handle_withdraw_funds,
)
from .state import PlayerState, build_state


def pack_event(event) -> AnyProto:
    """Pack an event into Any for state application."""
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")
    return event_any


def apply_event(state: PlayerState, event) -> PlayerState:
    """Pack and apply a single event to state."""
    return build_state(state, [pack_event(event)])


class TestRegister:
    """Test handle_register_player()."""

    def test_register_creates_player(self):
        state = PlayerState()
        assert not state.exists

        cmd = player.RegisterPlayer(
            display_name="Alice",
            email="alice@example.com",
            player_type=poker_types.PlayerType.HUMAN,
        )
        event = handle_register_player(cmd, state, seq=0)

        # Apply event to state
        apply_event(state, event)

        assert state.exists
        assert state.display_name == "Alice"
        assert state.email == "alice@example.com"
        assert state.bankroll == 0
        assert event.display_name == "Alice"

    def test_register_ai_player(self):
        state = PlayerState()
        cmd = player.RegisterPlayer(
            display_name="Bot-1",
            email="bot@ai.local",
            player_type=poker_types.PlayerType.AI,
            ai_model_id="gpt-poker-v1",
        )
        event = handle_register_player(cmd, state, seq=0)
        apply_event(state, event)

        assert state.player_type == poker_types.PlayerType.AI
        assert state.ai_model_id == "gpt-poker-v1"

    def test_register_rejects_existing_player(self):
        state = PlayerState()
        cmd = player.RegisterPlayer(display_name="Alice", email="a@b.com")
        event = handle_register_player(cmd, state, seq=0)
        apply_event(state, event)

        with pytest.raises(CommandRejectedError, match="already exists"):
            handle_register_player(cmd, state, seq=1)

    def test_register_requires_display_name(self):
        state = PlayerState()
        cmd = player.RegisterPlayer(email="a@b.com")

        with pytest.raises(CommandRejectedError, match="display_name"):
            handle_register_player(cmd, state, seq=0)

    def test_register_requires_email(self):
        state = PlayerState()
        cmd = player.RegisterPlayer(display_name="Alice")

        with pytest.raises(CommandRejectedError, match="email"):
            handle_register_player(cmd, state, seq=0)


class TestDeposit:
    """Test handle_deposit_funds()."""

    def test_deposit_increases_bankroll(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        cmd = player.DepositFunds(amount=poker_types.Currency(amount=500))
        event = handle_deposit_funds(cmd, state, seq=1)
        apply_event(state, event)

        assert state.bankroll == 500
        assert event.new_balance.amount == 500

    def test_multiple_deposits_accumulate(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        event1 = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=100)),
            state,
            seq=1,
        )
        apply_event(state, event1)

        event2 = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=50)),
            state,
            seq=2,
        )
        apply_event(state, event2)

        assert state.bankroll == 150

    def test_deposit_requires_existing_player(self):
        state = PlayerState()
        cmd = player.DepositFunds(amount=poker_types.Currency(amount=100))

        with pytest.raises(CommandRejectedError, match="does not exist"):
            handle_deposit_funds(cmd, state, seq=0)

    def test_deposit_requires_positive_amount(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        with pytest.raises(CommandRejectedError, match="positive"):
            handle_deposit_funds(
                player.DepositFunds(amount=poker_types.Currency(amount=0)),
                state,
                seq=1,
            )


class TestWithdraw:
    """Test handle_withdraw_funds()."""

    def test_withdraw_decreases_bankroll(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=500)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        event = handle_withdraw_funds(
            player.WithdrawFunds(amount=poker_types.Currency(amount=200)),
            state,
            seq=2,
        )
        apply_event(state, event)

        assert state.bankroll == 300
        assert event.new_balance.amount == 300

    def test_withdraw_rejects_insufficient_funds(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=100)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        with pytest.raises(CommandRejectedError, match="Insufficient"):
            handle_withdraw_funds(
                player.WithdrawFunds(amount=poker_types.Currency(amount=200)),
                state,
                seq=2,
            )


class TestReserve:
    """Test handle_reserve_funds()."""

    def test_reserve_tracks_table_reservation(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=1000)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        table_root = b"\x01\x02\x03\x04"
        cmd = player.ReserveFunds(
            table_root=table_root,
            amount=poker_types.Currency(amount=200),
        )
        event = handle_reserve_funds(cmd, state, seq=2)
        apply_event(state, event)

        assert state.reserved_funds == 200
        assert state.available_balance == 800
        assert table_root.hex() in state.table_reservations

    def test_reserve_rejects_duplicate_table(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=1000)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        table_root = b"\x01\x02\x03\x04"
        cmd = player.ReserveFunds(
            table_root=table_root,
            amount=poker_types.Currency(amount=200),
        )
        event = handle_reserve_funds(cmd, state, seq=2)
        apply_event(state, event)

        with pytest.raises(CommandRejectedError, match="already reserved"):
            handle_reserve_funds(cmd, state, seq=3)


class TestRelease:
    """Test handle_release_funds()."""

    def test_release_removes_reservation(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=1000)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        table_root = b"\x01\x02\x03\x04"
        res_event = handle_reserve_funds(
            player.ReserveFunds(
                table_root=table_root,
                amount=poker_types.Currency(amount=200),
            ),
            state,
            seq=2,
        )
        apply_event(state, res_event)

        event = handle_release_funds(
            player.ReleaseFunds(table_root=table_root),
            state,
            seq=3,
        )
        apply_event(state, event)

        assert state.reserved_funds == 0
        assert state.available_balance == 1000
        assert table_root.hex() not in state.table_reservations

    def test_release_rejects_no_reservation(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        with pytest.raises(CommandRejectedError, match="No funds reserved"):
            handle_release_funds(
                player.ReleaseFunds(table_root=b"\x01\x02\x03\x04"),
                state,
                seq=1,
            )


class TestStateAccessors:
    """Test state accessor properties."""

    def test_player_id_format(self):
        state = PlayerState()
        event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="alice@test.com"),
            state,
            seq=0,
        )
        apply_event(state, event)
        assert state.player_id == "player_alice@test.com"

    def test_player_type_returns_enum(self):
        state = PlayerState()
        event = handle_register_player(
            player.RegisterPlayer(
                display_name="Bot",
                email="bot@ai.local",
                player_type=poker_types.PlayerType.AI,
            ),
            state,
            seq=0,
        )
        apply_event(state, event)
        assert state.player_type == poker_types.PlayerType.AI

    def test_status_after_register(self):
        state = PlayerState()
        event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, event)
        assert state.status == "active"


class TestEventHandlers:
    """Test event handlers for saga-generated events."""

    def test_funds_transferred_updates_bankroll(self):
        """FundsTransferred event is generated by sagas, not commands."""
        # Build event book with registration + transfer events
        event_book = types.EventBook()

        # Add PlayerRegistered event
        registered = player.PlayerRegistered(
            display_name="Alice",
            email="alice@test.com",
        )
        registered_any = AnyProto()
        registered_any.Pack(registered, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=registered_any))

        # Add FundsTransferred event (from saga)
        transferred = player.FundsTransferred(
            new_balance=poker_types.Currency(amount=500, currency_code="CHIPS"),
        )
        transferred_any = AnyProto()
        transferred_any.Pack(transferred, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=transferred_any))

        # Create state from event book
        state = PlayerState()
        events = [page.event for page in event_book.pages]
        build_state(state, events)

        assert state.exists
        assert state.bankroll == 500


class TestEdgeCases:
    """Test edge cases for full coverage."""

    def test_withdraw_requires_existing_player(self):
        state = PlayerState()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            handle_withdraw_funds(
                player.WithdrawFunds(amount=poker_types.Currency(amount=100)),
                state,
                seq=0,
            )

    def test_withdraw_requires_positive_amount(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=100)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        with pytest.raises(CommandRejectedError, match="positive"):
            handle_withdraw_funds(
                player.WithdrawFunds(amount=poker_types.Currency(amount=0)),
                state,
                seq=2,
            )

    def test_reserve_requires_existing_player(self):
        state = PlayerState()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            handle_reserve_funds(
                player.ReserveFunds(
                    table_root=b"\x01\x02",
                    amount=poker_types.Currency(amount=100),
                ),
                state,
                seq=0,
            )

    def test_reserve_requires_positive_amount(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=100)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        with pytest.raises(CommandRejectedError, match="positive"):
            handle_reserve_funds(
                player.ReserveFunds(
                    table_root=b"\x01\x02",
                    amount=poker_types.Currency(amount=0),
                ),
                state,
                seq=2,
            )

    def test_reserve_rejects_insufficient_funds(self):
        state = PlayerState()
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)

        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=100)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)

        with pytest.raises(CommandRejectedError, match="Insufficient"):
            handle_reserve_funds(
                player.ReserveFunds(
                    table_root=b"\x01\x02",
                    amount=poker_types.Currency(amount=500),
                ),
                state,
                seq=2,
            )

    def test_release_requires_existing_player(self):
        state = PlayerState()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            handle_release_funds(
                player.ReleaseFunds(table_root=b"\x01\x02"),
                state,
                seq=0,
            )


class TestCompleteLifecycle:
    """Test complete player lifecycle."""

    def test_register_deposit_reserve_release_withdraw(self):
        state = PlayerState()

        # Register
        reg_event = handle_register_player(
            player.RegisterPlayer(display_name="Alice", email="a@b.com"),
            state,
            seq=0,
        )
        apply_event(state, reg_event)
        assert state.exists
        assert state.bankroll == 0

        # Deposit
        dep_event = handle_deposit_funds(
            player.DepositFunds(amount=poker_types.Currency(amount=1000)),
            state,
            seq=1,
        )
        apply_event(state, dep_event)
        assert state.bankroll == 1000

        # Reserve for table
        table_root = b"\xaa\xbb\xcc\xdd"
        res_event = handle_reserve_funds(
            player.ReserveFunds(
                table_root=table_root,
                amount=poker_types.Currency(amount=200),
            ),
            state,
            seq=2,
        )
        apply_event(state, res_event)
        assert state.reserved_funds == 200
        assert state.available_balance == 800

        # Release reservation
        rel_event = handle_release_funds(
            player.ReleaseFunds(table_root=table_root),
            state,
            seq=3,
        )
        apply_event(state, rel_event)
        assert state.reserved_funds == 0
        assert state.available_balance == 1000

        # Withdraw
        with_event = handle_withdraw_funds(
            player.WithdrawFunds(amount=poker_types.Currency(amount=300)),
            state,
            seq=4,
        )
        apply_event(state, with_event)
        assert state.bankroll == 700
