"""Release reservation command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from state import InventoryState
from handlers.errors import CommandRejectedError


def handle_release_reservation(command_book, command_any, state: InventoryState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = domains.ReleaseReservation()
    command_any.Unpack(cmd)

    if not cmd.order_id:
        raise CommandRejectedError("Order ID is required")
    if cmd.order_id not in state.reservations:
        raise CommandRejectedError("No reservation found for this order")

    qty = state.reservations[cmd.order_id]

    log.info("releasing_reservation", order_id=cmd.order_id, quantity=qty)

    event = domains.ReservationReleased(
        order_id=cmd.order_id,
        quantity=qty,
        new_available=state.available() + qty,
        released_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
