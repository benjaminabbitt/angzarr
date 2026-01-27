"""AddItem command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from .state import CartState

from .errors import CommandRejectedError, errmsg


def handle_add_item(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError(errmsg.CART_NOT_FOUND)
    if not state.is_active():
        raise CommandRejectedError(errmsg.CART_CHECKED_OUT)

    cmd = domains.AddItem()
    command_any.Unpack(cmd)

    if not cmd.product_id:
        raise CommandRejectedError(errmsg.PRODUCT_ID_REQUIRED)
    if cmd.quantity <= 0:
        raise CommandRejectedError(errmsg.QUANTITY_POSITIVE)

    new_subtotal = state.subtotal_cents + (cmd.quantity * cmd.unit_price_cents)

    log.info("adding_item", product_id=cmd.product_id, quantity=cmd.quantity)

    event = domains.ItemAdded(
        product_id=cmd.product_id,
        name=cmd.name,
        quantity=cmd.quantity,
        unit_price_cents=cmd.unit_price_cents,
        new_subtotal=new_subtotal,
        added_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
