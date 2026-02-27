"""Player command handlers for router pattern.

Each handler follows the guard/validate/compute pattern:
- guard: Check state preconditions (aggregate exists, correct phase)
- validate: Validate command inputs
- compute: Build the resulting event

Why this pattern?
-----------------
Each function is pure (state in, result out), enabling direct unit testing
without mocking infrastructure. You can test guard(), validate(), and compute()
independently by passing state structs and asserting on returned events.
The @command_handler decorator handles proto serialization separately.
"""

from angzarr_client import now
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player_proto
from angzarr_client.proto.examples import poker_types_pb2 as poker_types
from angzarr_client.router import command_handler

from .state import PlayerState


@command_handler(player_proto.RegisterPlayer)
def handle_register_player(
    cmd: player_proto.RegisterPlayer, state: PlayerState, seq: int
) -> player_proto.PlayerRegistered:
    """Register a new player."""
    # Guard
    if state.exists:
        raise CommandRejectedError("Player already exists")
    # Validate
    if not cmd.display_name:
        raise CommandRejectedError("display_name is required")
    if not cmd.email:
        raise CommandRejectedError("email is required")
    # Compute
    return player_proto.PlayerRegistered(
        display_name=cmd.display_name,
        email=cmd.email,
        player_type=cmd.player_type,
        ai_model_id=cmd.ai_model_id,
        registered_at=now(),
    )


@command_handler(player_proto.DepositFunds)
def handle_deposit_funds(
    cmd: player_proto.DepositFunds, state: PlayerState, seq: int
) -> player_proto.FundsDeposited:
    """Deposit funds into player's bankroll."""
    # docs:start:deposit_guard
    # Guard
    if not state.exists:
        raise CommandRejectedError("Player does not exist")
    # docs:end:deposit_guard
    # docs:start:deposit_validate
    # Validate
    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")
    # docs:end:deposit_validate
    # docs:start:deposit_compute
    # Compute
    new_balance = state.bankroll + amount
    return player_proto.FundsDeposited(
        amount=cmd.amount,
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        deposited_at=now(),
    )
    # docs:end:deposit_compute


@command_handler(player_proto.WithdrawFunds)
def handle_withdraw_funds(
    cmd: player_proto.WithdrawFunds, state: PlayerState, seq: int
) -> player_proto.FundsWithdrawn:
    """Withdraw funds from player's bankroll."""
    # Guard
    if not state.exists:
        raise CommandRejectedError("Player does not exist")
    # Validate
    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")
    if amount > state.available_balance:
        raise CommandRejectedError("Insufficient funds")
    # Compute
    new_balance = state.bankroll - amount
    return player_proto.FundsWithdrawn(
        amount=cmd.amount,
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        withdrawn_at=now(),
    )


# docs:start:reserve_funds_imp
@command_handler(player_proto.ReserveFunds)
def handle_reserve_funds(
    cmd: player_proto.ReserveFunds, state: PlayerState, seq: int
) -> player_proto.FundsReserved:
    """Reserve funds for a table buy-in."""
    # Guard
    if not state.exists:
        raise CommandRejectedError("Player does not exist")
    # Validate
    amount = cmd.amount.amount if cmd.amount else 0
    if amount <= 0:
        raise CommandRejectedError("amount must be positive")
    table_key = cmd.table_root.hex()
    if table_key in state.table_reservations:
        raise CommandRejectedError("Funds already reserved for this table")
    if amount > state.available_balance:
        raise CommandRejectedError("Insufficient funds")
    # Compute
    new_reserved = state.reserved_funds + amount
    new_available = state.bankroll - new_reserved
    return player_proto.FundsReserved(
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


# docs:end:reserve_funds_imp


@command_handler(player_proto.ReleaseFunds)
def handle_release_funds(
    cmd: player_proto.ReleaseFunds, state: PlayerState, seq: int
) -> player_proto.FundsReleased:
    """Release reserved funds when leaving a table."""
    # Guard
    if not state.exists:
        raise CommandRejectedError("Player does not exist")
    # Validate
    table_key = cmd.table_root.hex()
    reserved_for_table = state.table_reservations.get(table_key, 0)
    if reserved_for_table == 0:
        raise CommandRejectedError("No funds reserved for this table")
    # Compute
    new_reserved = state.reserved_funds - reserved_for_table
    new_available = state.bankroll - new_reserved
    return player_proto.FundsReleased(
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


@command_handler(player_proto.SitOut)
def handle_sit_out(
    cmd: player_proto.SitOut, state: PlayerState, seq: int
) -> player_proto.PlayerSittingOut:
    """Player chooses to sit out at a table."""
    # Guard
    if not state.exists:
        raise CommandRejectedError("Player does not exist")
    # Validate
    table_key = cmd.table_root.hex()
    if table_key not in state.table_reservations:
        raise CommandRejectedError("Player is not at this table")
    # Compute
    return player_proto.PlayerSittingOut(
        table_root=cmd.table_root,
        sat_out_at=now(),
    )


@command_handler(player_proto.SitIn)
def handle_sit_in(
    cmd: player_proto.SitIn, state: PlayerState, seq: int
) -> player_proto.PlayerReturningToPlay:
    """Player chooses to return to play at a table."""
    # Guard
    if not state.exists:
        raise CommandRejectedError("Player does not exist")
    # Validate
    table_key = cmd.table_root.hex()
    if table_key not in state.table_reservations:
        raise CommandRejectedError("Player is not at this table")
    # Compute
    return player_proto.PlayerReturningToPlay(
        table_root=cmd.table_root,
        sat_in_at=now(),
    )


__all__ = [
    "handle_register_player",
    "handle_deposit_funds",
    "handle_withdraw_funds",
    "handle_reserve_funds",
    "handle_release_funds",
    "handle_sit_out",
    "handle_sit_in",
]
