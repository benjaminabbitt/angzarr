"""Tests for Player aggregate."""

import pytest

from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import types_pb2 as poker_types

from .player import Player


class TestRegister:
    """Test Player.register()."""

    def test_register_creates_player(self):
        p = Player()
        assert not p.exists

        cmd = player.RegisterPlayer(
            display_name="Alice",
            email="alice@example.com",
            player_type=poker_types.PlayerType.HUMAN,
        )
        event = p.register(cmd)

        assert p.exists
        assert p.display_name == "Alice"
        assert p.email == "alice@example.com"
        assert p.bankroll == 0
        assert event.display_name == "Alice"

    def test_register_ai_player(self):
        p = Player()
        cmd = player.RegisterPlayer(
            display_name="Bot-1",
            email="bot@ai.local",
            player_type=poker_types.PlayerType.AI,
            ai_model_id="gpt-poker-v1",
        )
        p.register(cmd)

        assert p.is_ai
        assert p.ai_model_id == "gpt-poker-v1"

    def test_register_rejects_existing_player(self):
        p = Player()
        cmd = player.RegisterPlayer(display_name="Alice", email="a@b.com")
        p.register(cmd)

        with pytest.raises(CommandRejectedError, match="already exists"):
            p.register(cmd)

    def test_register_requires_display_name(self):
        p = Player()
        cmd = player.RegisterPlayer(email="a@b.com")

        with pytest.raises(CommandRejectedError, match="display_name"):
            p.register(cmd)

    def test_register_requires_email(self):
        p = Player()
        cmd = player.RegisterPlayer(display_name="Alice")

        with pytest.raises(CommandRejectedError, match="email"):
            p.register(cmd)


class TestDeposit:
    """Test Player.deposit()."""

    def test_deposit_increases_bankroll(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))

        cmd = player.DepositFunds(amount=poker_types.Currency(amount=500))
        event = p.deposit(cmd)

        assert p.bankroll == 500
        assert event.new_balance.amount == 500

    def test_multiple_deposits_accumulate(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))

        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=100)))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=50)))

        assert p.bankroll == 150

    def test_deposit_requires_existing_player(self):
        p = Player()
        cmd = player.DepositFunds(amount=poker_types.Currency(amount=100))

        with pytest.raises(CommandRejectedError, match="does not exist"):
            p.deposit(cmd)

    def test_deposit_requires_positive_amount(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))

        with pytest.raises(CommandRejectedError, match="positive"):
            p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=0)))


class TestWithdraw:
    """Test Player.withdraw()."""

    def test_withdraw_decreases_bankroll(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=500)))

        event = p.withdraw(player.WithdrawFunds(amount=poker_types.Currency(amount=200)))

        assert p.bankroll == 300
        assert event.new_balance.amount == 300

    def test_withdraw_rejects_insufficient_funds(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=100)))

        with pytest.raises(CommandRejectedError, match="Insufficient"):
            p.withdraw(player.WithdrawFunds(amount=poker_types.Currency(amount=200)))


class TestReserve:
    """Test Player.reserve()."""

    def test_reserve_tracks_table_reservation(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=1000)))

        table_root = b"\x01\x02\x03\x04"
        cmd = player.ReserveFunds(
            table_root=table_root,
            amount=poker_types.Currency(amount=200),
        )
        p.reserve(cmd)

        assert p.reserved_funds == 200
        assert p.available_balance == 800
        assert table_root.hex() in p.table_reservations

    def test_reserve_rejects_duplicate_table(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=1000)))

        table_root = b"\x01\x02\x03\x04"
        cmd = player.ReserveFunds(
            table_root=table_root,
            amount=poker_types.Currency(amount=200),
        )
        p.reserve(cmd)

        with pytest.raises(CommandRejectedError, match="already reserved"):
            p.reserve(cmd)


class TestRelease:
    """Test Player.release()."""

    def test_release_removes_reservation(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=1000)))

        table_root = b"\x01\x02\x03\x04"
        p.reserve(player.ReserveFunds(
            table_root=table_root,
            amount=poker_types.Currency(amount=200),
        ))

        p.release(player.ReleaseFunds(table_root=table_root))

        assert p.reserved_funds == 0
        assert p.available_balance == 1000
        assert table_root.hex() not in p.table_reservations

    def test_release_rejects_no_reservation(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))

        with pytest.raises(CommandRejectedError, match="No funds reserved"):
            p.release(player.ReleaseFunds(table_root=b"\x01\x02\x03\x04"))


class TestStateAccessors:
    """Test state accessor properties."""

    def test_player_id_format(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="alice@test.com"))
        assert p.player_id == "player_alice@test.com"

    def test_player_type_returns_enum(self):
        p = Player()
        p.register(player.RegisterPlayer(
            display_name="Bot",
            email="bot@ai.local",
            player_type=poker_types.PlayerType.AI,
        ))
        assert p.player_type == poker_types.PlayerType.AI

    def test_status_after_register(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        assert p.status == "active"


class TestEventHandlers:
    """Test event handlers for saga-generated events."""

    def test_funds_transferred_updates_bankroll(self):
        """FundsTransferred event is generated by sagas, not commands."""
        from google.protobuf.any_pb2 import Any
        from angzarr_client.proto.angzarr import types_pb2 as types

        # Build event book with registration + transfer events
        event_book = types.EventBook()

        # Add PlayerRegistered event
        registered = player.PlayerRegistered(
            display_name="Alice",
            email="alice@test.com",
        )
        registered_any = Any()
        registered_any.Pack(registered, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=registered_any))

        # Add FundsTransferred event (from saga)
        transferred = player.FundsTransferred(
            new_balance=poker_types.Currency(amount=500, currency_code="CHIPS"),
        )
        transferred_any = Any()
        transferred_any.Pack(transferred, type_url_prefix="type.googleapis.com/")
        event_book.pages.append(types.EventPage(event=transferred_any))

        # Create player from event book
        p = Player(event_book)

        assert p.exists
        assert p.bankroll == 500


class TestEdgeCases:
    """Test edge cases for full coverage."""

    def test_withdraw_requires_existing_player(self):
        p = Player()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            p.withdraw(player.WithdrawFunds(amount=poker_types.Currency(amount=100)))

    def test_withdraw_requires_positive_amount(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=100)))
        with pytest.raises(CommandRejectedError, match="positive"):
            p.withdraw(player.WithdrawFunds(amount=poker_types.Currency(amount=0)))

    def test_reserve_requires_existing_player(self):
        p = Player()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            p.reserve(player.ReserveFunds(
                table_root=b"\x01\x02",
                amount=poker_types.Currency(amount=100),
            ))

    def test_reserve_requires_positive_amount(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=100)))
        with pytest.raises(CommandRejectedError, match="positive"):
            p.reserve(player.ReserveFunds(
                table_root=b"\x01\x02",
                amount=poker_types.Currency(amount=0),
            ))

    def test_reserve_rejects_insufficient_funds(self):
        p = Player()
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=100)))
        with pytest.raises(CommandRejectedError, match="Insufficient"):
            p.reserve(player.ReserveFunds(
                table_root=b"\x01\x02",
                amount=poker_types.Currency(amount=500),
            ))

    def test_release_requires_existing_player(self):
        p = Player()
        with pytest.raises(CommandRejectedError, match="does not exist"):
            p.release(player.ReleaseFunds(table_root=b"\x01\x02"))


class TestCompleteLifecycle:
    """Test complete player lifecycle."""

    def test_register_deposit_reserve_release_withdraw(self):
        p = Player()

        # Register
        p.register(player.RegisterPlayer(display_name="Alice", email="a@b.com"))
        assert p.exists
        assert p.bankroll == 0

        # Deposit
        p.deposit(player.DepositFunds(amount=poker_types.Currency(amount=1000)))
        assert p.bankroll == 1000

        # Reserve for table
        table_root = b"\xaa\xbb\xcc\xdd"
        p.reserve(player.ReserveFunds(
            table_root=table_root,
            amount=poker_types.Currency(amount=200),
        ))
        assert p.reserved_funds == 200
        assert p.available_balance == 800

        # Release reservation
        p.release(player.ReleaseFunds(table_root=table_root))
        assert p.reserved_funds == 0
        assert p.available_balance == 1000

        # Withdraw
        p.withdraw(player.WithdrawFunds(amount=poker_types.Currency(amount=300)))
        assert p.bankroll == 700
