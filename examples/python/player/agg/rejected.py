"""Rejection handlers for saga/PM compensation.

Handles command rejections that require compensating actions.
"""

from angzarr_client import now
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

from .state import PlayerState


# docs:start:rejected_handler
def handle_join_rejected(
    notification: types.Notification,
    state: PlayerState,
) -> types.EventBook | None:
    """Handle JoinTable rejection by releasing reserved funds.

    Called when the JoinTable command (issued by saga-player-table after
    FundsReserved) is rejected by the Table aggregate.
    """
    from google.protobuf.any_pb2 import Any

    # Extract rejection details from the notification payload
    rejection = types.RejectionNotification()
    if notification.payload:
        notification.payload.Unpack(rejection)

    # Extract table_root from the rejected command
    table_root = b""
    if rejection.rejected_command and rejection.rejected_command.cover:
        if rejection.rejected_command.cover.root:
            table_root = rejection.rejected_command.cover.root.value

    # Release the funds that were reserved for this table
    table_key = table_root.hex()
    reserved_amount = state.table_reservations.get(table_key, 0)
    new_reserved = state.reserved_funds - reserved_amount
    new_available = state.bankroll - new_reserved

    event = player.FundsReleased(
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

    # Pack the event
    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")

    # Build the EventBook using the notification's cover for routing
    return types.EventBook(
        cover=notification.cover,
        pages=[types.EventPage(sequence=0, event=event_any)],
    )


# docs:end:rejected_handler
