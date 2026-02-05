"""Handler for ConfirmPayment command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import order_pb2 as order

from .state import OrderState


def handle_confirm_payment(
    command_book: types.CommandBook,
    command_any: Any,
    state: OrderState,
    seq: int,
) -> types.EventBook:
    """Handle ConfirmPayment command."""
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_payment_submitted():
        raise CommandRejectedError("Payment not submitted")

    cmd = order.ConfirmPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_reference:
        raise CommandRejectedError("Payment reference is required")

    # 1 point per dollar
    loyalty_points_earned = state.total_after_discount() // 100

    event = order.OrderCompleted(
        final_total_cents=state.total_after_discount(),
        payment_method=state.payment_method,
        payment_reference=cmd.payment_reference,
        loyalty_points_earned=loyalty_points_earned,
        completed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        customer_root=state.customer_root,
        cart_root=state.cart_root,
    )
    event.items.extend(
        order.LineItem(
            product_id=i.product_id,
            name=i.name,
            quantity=i.quantity,
            unit_price_cents=i.unit_price_cents,
        )
        for i in state.items
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
