"""Order state rebuilding logic."""

from dataclasses import dataclass, field

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from angzarr.state_builder import StateBuilder
from proto import order_pb2 as order


@dataclass
class LineItem:
    product_id: str
    name: str
    quantity: int
    unit_price_cents: int


@dataclass
class OrderState:
    customer_id: str = ""
    items: list = field(default_factory=list)
    subtotal_cents: int = 0
    discount_cents: int = 0
    loyalty_points_used: int = 0
    payment_method: str = ""
    payment_reference: str = ""
    status: str = ""
    customer_root: bytes = b""
    cart_root: bytes = b""

    def exists(self) -> bool:
        return bool(self.customer_id)

    def is_pending(self) -> bool:
        return self.status == "pending"

    def is_payment_submitted(self) -> bool:
        return self.status == "payment_submitted"

    def is_completed(self) -> bool:
        return self.status == "completed"

    def is_cancelled(self) -> bool:
        return self.status == "cancelled"

    def total_after_discount(self) -> int:
        return self.subtotal_cents - self.discount_cents


# ============================================================================
# Named event appliers
# ============================================================================


def apply_order_created(state: OrderState, event: AnyProto) -> None:
    e = order.OrderCreated()
    event.Unpack(e)
    state.customer_id = e.customer_id
    state.subtotal_cents = e.subtotal_cents
    state.status = "pending"
    state.items = [
        LineItem(i.product_id, i.name, i.quantity, i.unit_price_cents)
        for i in e.items
    ]
    state.customer_root = e.customer_root
    state.cart_root = e.cart_root


def apply_loyalty_discount(state: OrderState, event: AnyProto) -> None:
    e = order.LoyaltyDiscountApplied()
    event.Unpack(e)
    state.loyalty_points_used = e.points_used
    state.discount_cents = e.discount_cents


def apply_payment_submitted(state: OrderState, event: AnyProto) -> None:
    e = order.PaymentSubmitted()
    event.Unpack(e)
    state.payment_method = e.payment_method
    state.status = "payment_submitted"


def apply_order_completed(state: OrderState, event: AnyProto) -> None:
    e = order.OrderCompleted()
    event.Unpack(e)
    state.payment_reference = e.payment_reference
    state.status = "completed"


def apply_order_cancelled(state: OrderState, _event: AnyProto) -> None:
    state.status = "cancelled"


# ============================================================================
# State rebuilding
# ============================================================================

# stateBuilder is the single source of truth for event type -> applier mapping.
_state_builder = (
    StateBuilder(OrderState)
    .on("OrderCreated", apply_order_created)
    .on("LoyaltyDiscountApplied", apply_loyalty_discount)
    .on("PaymentSubmitted", apply_payment_submitted)
    .on("OrderCompleted", apply_order_completed)
    .on("OrderCancelled", apply_order_cancelled)
)


def rebuild_state(event_book: types.EventBook | None) -> OrderState:
    """Rebuild order state from events."""
    return _state_builder.rebuild(event_book)


def apply_event(state: OrderState, event: AnyProto) -> None:
    """Apply a single event to the order state."""
    _state_builder.apply(state, event)
