"""Inventory state rebuilding logic."""

from dataclasses import dataclass, field

from angzarr import types_pb2 as types
from proto import domains_pb2 as domains


@dataclass
class InventoryState:
    product_id: str = ""
    on_hand: int = 0
    reserved: int = 0
    low_stock_threshold: int = 0
    reservations: dict = field(default_factory=dict)  # order_id -> quantity

    def exists(self) -> bool:
        return bool(self.product_id)

    def available(self) -> int:
        return self.on_hand - self.reserved

    def is_low_stock(self) -> bool:
        return self.available() < self.low_stock_threshold


def rebuild_state(event_book: types.EventBook | None) -> InventoryState:
    """Rebuild inventory state from events."""
    state = InventoryState()

    if event_book is None or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("StockInitialized"):
            event = domains.StockInitialized()
            page.event.Unpack(event)
            state.product_id = event.product_id
            state.on_hand = event.quantity
            state.low_stock_threshold = event.low_stock_threshold

        elif page.event.type_url.endswith("StockReceived"):
            event = domains.StockReceived()
            page.event.Unpack(event)
            state.on_hand = event.new_on_hand

        elif page.event.type_url.endswith("StockReserved"):
            event = domains.StockReserved()
            page.event.Unpack(event)
            state.reserved += event.quantity
            state.reservations[event.order_id] = event.quantity

        elif page.event.type_url.endswith("ReservationReleased"):
            event = domains.ReservationReleased()
            page.event.Unpack(event)
            qty = state.reservations.pop(event.order_id, 0)
            state.reserved -= qty

        elif page.event.type_url.endswith("ReservationCommitted"):
            event = domains.ReservationCommitted()
            page.event.Unpack(event)
            qty = state.reservations.pop(event.order_id, 0)
            state.on_hand -= qty
            state.reserved -= qty

    return state
