"""Handler for CommitReservation command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import domains_pb2 as domains

from handlers.state import InventoryState


def handle_commit_reservation(
    command_book: types.CommandBook,
    command_any: Any,
    state: InventoryState,
    seq: int,
) -> types.EventBook:
    """Handle CommitReservation command."""
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = domains.CommitReservation()
    command_any.Unpack(cmd)

    if not cmd.order_id:
        raise CommandRejectedError("Order ID is required")
    if cmd.order_id not in state.reservations:
        raise CommandRejectedError("No reservation found for this order")

    qty = state.reservations[cmd.order_id]

    event = domains.ReservationCommitted(
        order_id=cmd.order_id,
        quantity=qty,
        new_on_hand=state.on_hand - qty,
        committed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return types.EventBook(
        cover=command_book.cover,
        pages=[
            types.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )
