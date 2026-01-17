"""Receive stock command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from state import InventoryState
from handlers.errors import CommandRejectedError


def handle_receive_stock(command_book, command_any, state: InventoryState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Inventory not initialized")

    cmd = domains.ReceiveStock()
    command_any.Unpack(cmd)

    if cmd.quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")

    log.info("receiving_stock", quantity=cmd.quantity, reference=cmd.reference)

    event = domains.StockReceived(
        quantity=cmd.quantity,
        new_on_hand=state.on_hand + cmd.quantity,
        reference=cmd.reference,
        received_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
