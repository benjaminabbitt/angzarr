"""Order state rebuilding logic."""

from dataclasses import dataclass, field

from angzarr import types_pb2 as types
from proto import domains_pb2 as domains


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


def rebuild_state(event_book: types.EventBook | None) -> OrderState:
    """Rebuild order state from events."""
    state = OrderState()

    if event_book is None or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("OrderCreated"):
            event = domains.OrderCreated()
            page.event.Unpack(event)
            state.customer_id = event.customer_id
            state.subtotal_cents = event.subtotal_cents
            state.status = "pending"
            state.items = [
                LineItem(i.product_id, i.name, i.quantity, i.unit_price_cents)
                for i in event.items
            ]

        elif page.event.type_url.endswith("LoyaltyDiscountApplied"):
            event = domains.LoyaltyDiscountApplied()
            page.event.Unpack(event)
            state.loyalty_points_used = event.points_used
            state.discount_cents = event.discount_cents

        elif page.event.type_url.endswith("PaymentSubmitted"):
            event = domains.PaymentSubmitted()
            page.event.Unpack(event)
            state.payment_method = event.payment_method
            state.status = "payment_submitted"

        elif page.event.type_url.endswith("OrderCompleted"):
            event = domains.OrderCompleted()
            page.event.Unpack(event)
            state.payment_reference = event.payment_reference
            state.status = "completed"

        elif page.event.type_url.endswith("OrderCancelled"):
            state.status = "cancelled"

    return state
