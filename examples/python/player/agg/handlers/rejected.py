"""Rejected command handlers for Player aggregate.

When a saga-issued command (like JoinTable) is rejected by the target
aggregate, the framework sends a Notification back to the source aggregate.
Use @rejected decorators to handle these rejections and emit compensation events.
"""

from angzarr_client import now, rejected
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player_proto
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from .state import PlayerState


# docs:start:rejected_handler
@rejected(domain="table", command="JoinTable")
def handle_join_rejected(
    notification: types.Notification, state: PlayerState, seq: int
) -> player_proto.FundsReleased:
    """Handle JoinTable rejection by releasing reserved funds.

    Called when the JoinTable command (issued by saga-player-table after
    FundsReserved) is rejected by the Table aggregate.
    """
    # Extract table_root from the original command that was rejected
    table_root = notification.cover.root

    # Get the amount reserved for this table
    table_key = table_root.hex()
    reserved_amount = state.table_reservations.get(table_key, 0)

    # Compute new balances after release
    new_reserved = state.reserved_funds - reserved_amount
    new_available = state.bankroll - new_reserved

    return player_proto.FundsReleased(
        amount=poker_types.Currency(amount=reserved_amount, currency_code="CHIPS"),
        table_root=table_root,
        new_available_balance=poker_types.Currency(
            amount=new_available, currency_code="CHIPS"
        ),
        new_reserved_balance=poker_types.Currency(
            amount=new_reserved, currency_code="CHIPS"
        ),
        released_at=now(),
    )


# docs:end:rejected_handler


__all__ = ["handle_join_rejected"]
