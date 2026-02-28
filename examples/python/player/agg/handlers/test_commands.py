"""Unit tests for Player command handlers.

These tests demonstrate direct unit testing of the guard/validate/compute
pattern. Each handler is a pure function that takes (cmd, state, seq) and
returns an event or raises CommandRejectedError.

No mocking required - just create state, build command, call handler, assert.
"""

import pytest

from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player_proto
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from .commands import (
    handle_deposit_funds,
    handle_register_player,
    handle_reserve_funds,
    handle_withdraw_funds,
)
from .state import PlayerState


# docs:start:unit_test_deposit
class TestDepositFunds:
    """Test deposit handler - demonstrates guard/validate/compute testing."""

    def test_deposit_increases_bankroll(self):
        """Depositing funds increases the player's bankroll."""
        state = PlayerState(player_id="player_1", bankroll=1000)
        cmd = player_proto.DepositFunds(
            amount=poker_types.Currency(amount=500, currency_code="CHIPS")
        )

        event = handle_deposit_funds(cmd, state, seq=1)

        assert event.new_balance.amount == 1500

    def test_deposit_rejects_non_existent_player(self):
        """Cannot deposit to a player that doesn't exist (guard)."""
        state = PlayerState()  # player_id empty = doesn't exist
        cmd = player_proto.DepositFunds(
            amount=poker_types.Currency(amount=500, currency_code="CHIPS")
        )

        with pytest.raises(CommandRejectedError, match="does not exist"):
            handle_deposit_funds(cmd, state, seq=1)

    def test_deposit_rejects_zero_amount(self):
        """Cannot deposit zero or negative amount (validate)."""
        state = PlayerState(player_id="player_1", bankroll=1000)
        cmd = player_proto.DepositFunds(
            amount=poker_types.Currency(amount=0, currency_code="CHIPS")
        )

        with pytest.raises(CommandRejectedError, match="positive"):
            handle_deposit_funds(cmd, state, seq=1)


# docs:end:unit_test_deposit


# docs:start:unit_test_withdraw
class TestWithdrawFunds:
    """Test withdraw handler - demonstrates insufficient funds validation."""

    def test_withdraw_decreases_bankroll(self):
        """Withdrawing funds decreases the player's bankroll."""
        state = PlayerState(player_id="player_1", bankroll=1000)
        cmd = player_proto.WithdrawFunds(
            amount=poker_types.Currency(amount=400, currency_code="CHIPS")
        )

        event = handle_withdraw_funds(cmd, state, seq=1)

        assert event.new_balance.amount == 600

    def test_withdraw_rejects_insufficient_funds(self):
        """Cannot withdraw more than available balance."""
        state = PlayerState(player_id="player_1", bankroll=500)
        cmd = player_proto.WithdrawFunds(
            amount=poker_types.Currency(amount=600, currency_code="CHIPS")
        )

        with pytest.raises(CommandRejectedError, match="Insufficient"):
            handle_withdraw_funds(cmd, state, seq=1)


# docs:end:unit_test_withdraw


class TestReserveFunds:
    """Test fund reservation - demonstrates table-specific state tracking."""

    def test_reserve_locks_funds_for_table(self):
        """Reserving funds decreases available balance."""
        state = PlayerState(player_id="player_1", bankroll=1000)
        table_root = bytes.fromhex("deadbeef")
        cmd = player_proto.ReserveFunds(
            amount=poker_types.Currency(amount=500, currency_code="CHIPS"),
            table_root=table_root,
        )

        event = handle_reserve_funds(cmd, state, seq=1)

        assert event.new_available_balance.amount == 500
        assert event.new_reserved_balance.amount == 500

    def test_cannot_reserve_twice_for_same_table(self):
        """Cannot double-reserve for the same table."""
        table_root = bytes.fromhex("deadbeef")
        state = PlayerState(
            player_id="player_1",
            bankroll=1000,
            table_reservations={table_root.hex(): 500},
        )
        cmd = player_proto.ReserveFunds(
            amount=poker_types.Currency(amount=200, currency_code="CHIPS"),
            table_root=table_root,
        )

        with pytest.raises(CommandRejectedError, match="already reserved"):
            handle_reserve_funds(cmd, state, seq=1)


class TestRegisterPlayer:
    """Test player registration - demonstrates idempotency guard."""

    def test_register_creates_player(self):
        """Registering creates a new player."""
        state = PlayerState()
        cmd = player_proto.RegisterPlayer(
            display_name="Alice",
            email="alice@example.com",
        )

        event = handle_register_player(cmd, state, seq=0)

        assert event.display_name == "Alice"
        assert event.email == "alice@example.com"

    def test_cannot_register_twice(self):
        """Cannot register an already existing player."""
        state = PlayerState(player_id="player_1")
        cmd = player_proto.RegisterPlayer(
            display_name="Bob",
            email="bob@example.com",
        )

        with pytest.raises(CommandRejectedError, match="already exists"):
            handle_register_player(cmd, state, seq=1)
