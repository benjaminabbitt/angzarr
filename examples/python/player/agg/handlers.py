"""Player command handlers - functional pattern using @command_handler.

This file defines command handlers as pure functions decorated with
@command_handler. Contrasts with the OO pattern in player/agg/handlers/player.py
which uses @handles decorators on class methods.

Handler signature (decorated):
    handler(cmd: ConcreteCommand, state: PlayerState, seq: int) -> Event

The decorator auto-unpacks the command and packs the returned event.
"""

from angzarr_client import command_handler, now
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from .state import PlayerState


@command_handler(player.RegisterPlayer)
def handle_register(
    cmd: player.RegisterPlayer, state: PlayerState, seq: int
) -> player.PlayerRegistered:
    """Register a new player."""
    if state.exists:
        raise CommandRejectedError("Player already exists")
    if not cmd.display_name:
        raise CommandRejectedError("display_name is required")
    if not cmd.email:
        raise CommandRejectedError("email is required")

    return player.PlayerRegistered(
        display_name=cmd.display_name,
        email=cmd.email,
        player_type=cmd.player_type,
        ai_model_id=cmd.ai_model_id,
        registered_at=now(),
    )


@command_handler(player.DepositFunds)
def handle_deposit(
    cmd: player.DepositFunds, state: PlayerState, seq: int
) -> player.FundsDeposited:
    """Deposit funds into player's bankroll."""
    if not state.exists:
        raise CommandRejectedError("Player does not exist")

    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")

    new_balance = state.bankroll + amount
    return player.FundsDeposited(
        amount=cmd.amount,
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        deposited_at=now(),
    )


@command_handler(player.WithdrawFunds)
def handle_withdraw(
    cmd: player.WithdrawFunds, state: PlayerState, seq: int
) -> player.FundsWithdrawn:
    """Withdraw funds from player's bankroll."""
    if not state.exists:
        raise CommandRejectedError("Player does not exist")

    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")
    if amount > state.available_balance:
        raise CommandRejectedError("Insufficient funds")

    new_balance = state.bankroll - amount
    return player.FundsWithdrawn(
        amount=cmd.amount,
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        withdrawn_at=now(),
    )


@command_handler(player.ReserveFunds)
def handle_reserve(
    cmd: player.ReserveFunds, state: PlayerState, seq: int
) -> player.FundsReserved:
    """Reserve funds for a table buy-in."""
    if not state.exists:
        raise CommandRejectedError("Player does not exist")

    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")

    table_key = cmd.table_root.hex()
    if table_key in state.table_reservations:
        raise CommandRejectedError("Funds already reserved for this table")

    if amount > state.available_balance:
        raise CommandRejectedError("Insufficient funds")

    new_reserved = state.reserved_funds + amount
    new_available = state.bankroll - new_reserved
    return player.FundsReserved(
        amount=cmd.amount,
        table_root=cmd.table_root,
        new_available_balance=poker_types.Currency(
            amount=new_available, currency_code="CHIPS"
        ),
        new_reserved_balance=poker_types.Currency(
            amount=new_reserved, currency_code="CHIPS"
        ),
        reserved_at=now(),
    )


@command_handler(player.ReleaseFunds)
def handle_release(
    cmd: player.ReleaseFunds, state: PlayerState, seq: int
) -> player.FundsReleased:
    """Release reserved funds when leaving a table."""
    if not state.exists:
        raise CommandRejectedError("Player does not exist")

    table_key = cmd.table_root.hex()
    reserved_for_table = state.table_reservations.get(table_key, 0)
    if reserved_for_table == 0:
        raise CommandRejectedError("No funds reserved for this table")

    new_reserved = state.reserved_funds - reserved_for_table
    new_available = state.bankroll - new_reserved
    return player.FundsReleased(
        amount=poker_types.Currency(amount=reserved_for_table, currency_code="CHIPS"),
        table_root=cmd.table_root,
        new_available_balance=poker_types.Currency(
            amount=new_available, currency_code="CHIPS"
        ),
        new_reserved_balance=poker_types.Currency(
            amount=new_reserved, currency_code="CHIPS"
        ),
        released_at=now(),
    )
