"""Cart state management and event replay."""

from dataclasses import dataclass, field

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains


@dataclass
class CartItem:
    product_id: str
    name: str
    quantity: int
    unit_price_cents: int


@dataclass
class CartState:
    customer_id: str = ""
    items: dict = field(default_factory=dict)  # product_id -> CartItem
    subtotal_cents: int = 0
    coupon_code: str = ""
    discount_cents: int = 0
    status: str = ""

    def exists(self) -> bool:
        return bool(self.customer_id)

    def is_active(self) -> bool:
        return self.status == "active"


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> CartState:
    state = CartState()

    if event_book is None or not event_book.pages:
        return state

    if event_book.snapshot and event_book.snapshot.state:
        snap = domains.CartState()
        snap.ParseFromString(event_book.snapshot.state.value)
        state.customer_id = snap.customer_id
        state.subtotal_cents = snap.subtotal_cents
        state.coupon_code = snap.coupon_code
        state.discount_cents = snap.discount_cents
        state.status = snap.status
        for item in snap.items:
            state.items[item.product_id] = CartItem(
                product_id=item.product_id,
                name=item.name,
                quantity=item.quantity,
                unit_price_cents=item.unit_price_cents,
            )

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("CartCreated"):
            event = domains.CartCreated()
            page.event.Unpack(event)
            state.customer_id = event.customer_id
            state.status = "active"

        elif page.event.type_url.endswith("ItemAdded"):
            event = domains.ItemAdded()
            page.event.Unpack(event)
            state.items[event.product_id] = CartItem(
                product_id=event.product_id,
                name=event.name,
                quantity=event.quantity,
                unit_price_cents=event.unit_price_cents,
            )
            state.subtotal_cents = event.new_subtotal

        elif page.event.type_url.endswith("QuantityUpdated"):
            event = domains.QuantityUpdated()
            page.event.Unpack(event)
            if event.product_id in state.items:
                state.items[event.product_id].quantity = event.new_quantity
            state.subtotal_cents = event.new_subtotal

        elif page.event.type_url.endswith("ItemRemoved"):
            event = domains.ItemRemoved()
            page.event.Unpack(event)
            state.items.pop(event.product_id, None)
            state.subtotal_cents = event.new_subtotal

        elif page.event.type_url.endswith("CouponApplied"):
            event = domains.CouponApplied()
            page.event.Unpack(event)
            state.coupon_code = event.coupon_code
            state.discount_cents = event.discount_cents

        elif page.event.type_url.endswith("CartCleared"):
            state.items.clear()
            state.subtotal_cents = 0
            state.coupon_code = ""
            state.discount_cents = 0

        elif page.event.type_url.endswith("CartCheckedOut"):
            state.status = "checked_out"

    return state
