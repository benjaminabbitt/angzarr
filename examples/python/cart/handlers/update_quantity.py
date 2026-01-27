"""UpdateQuantity command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from .state import CartState

from .errors import CommandRejectedError, errmsg


def handle_update_quantity(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError(errmsg.CART_NOT_FOUND)
    if not state.is_active():
        raise CommandRejectedError(errmsg.CART_CHECKED_OUT)

    cmd = domains.UpdateQuantity()
    command_any.Unpack(cmd)

    if cmd.product_id not in state.items:
        raise CommandRejectedError(errmsg.ITEM_NOT_IN_CART)
    if cmd.new_quantity <= 0:
        raise CommandRejectedError(errmsg.QUANTITY_POSITIVE)

    item = state.items[cmd.product_id]
    old_subtotal = item.quantity * item.unit_price_cents
    new_item_subtotal = cmd.new_quantity * item.unit_price_cents
    new_subtotal = state.subtotal_cents - old_subtotal + new_item_subtotal

    log.info("updating_quantity", product_id=cmd.product_id, new_quantity=cmd.new_quantity)

    event = domains.QuantityUpdated(
        product_id=cmd.product_id,
        old_quantity=item.quantity,
        new_quantity=cmd.new_quantity,
        new_subtotal=new_subtotal,
        updated_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
