"""RemoveItem command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from state import CartState

from .errors import CommandRejectedError


def handle_remove_item(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    cmd = domains.RemoveItem()
    command_any.Unpack(cmd)

    if cmd.product_id not in state.items:
        raise CommandRejectedError("Item not in cart")

    item = state.items[cmd.product_id]
    item_subtotal = item.quantity * item.unit_price_cents
    new_subtotal = state.subtotal_cents - item_subtotal

    log.info("removing_item", product_id=cmd.product_id)

    event = domains.ItemRemoved(
        product_id=cmd.product_id,
        quantity=item.quantity,
        new_subtotal=new_subtotal,
        removed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
