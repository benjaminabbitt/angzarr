"""Handler for CreateOrder command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import order_pb2 as order

from .state import OrderState


def handle_create_order(
    command_book: types.CommandBook,
    command_any: Any,
    state: OrderState,
    seq: int,
) -> types.EventBook:
    """Handle CreateOrder command."""
    if state.exists():
        raise CommandRejectedError("Order already exists")

    cmd = order.CreateOrder()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("Customer ID is required")
    if not cmd.items:
        raise CommandRejectedError("Order must have at least one item")
    for item in cmd.items:
        if item.quantity <= 0:
            raise CommandRejectedError("Item quantity must be positive")

    subtotal = sum(item.quantity * item.unit_price_cents for item in cmd.items)

    event = order.OrderCreated(
        customer_id=cmd.customer_id,
        subtotal_cents=subtotal,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        customer_root=cmd.customer_root,
        cart_root=cmd.cart_root,
    )
    event.items.extend(cmd.items)

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
