"""Inventory state rebuilding logic."""

from dataclasses import dataclass, field

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from angzarr.state_builder import StateBuilder
from proto import inventory_pb2 as inventory


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


# ============================================================================
# Named event appliers
# ============================================================================


def apply_stock_initialized(state: InventoryState, event: AnyProto) -> None:
    e = inventory.StockInitialized()
    event.Unpack(e)
    state.product_id = e.product_id
    state.on_hand = e.quantity
    state.reserved = 0
    state.low_stock_threshold = e.low_stock_threshold
    state.reservations = {}


def apply_stock_received(state: InventoryState, event: AnyProto) -> None:
    e = inventory.StockReceived()
    event.Unpack(e)
    state.on_hand = e.new_on_hand


def apply_stock_reserved(state: InventoryState, event: AnyProto) -> None:
    e = inventory.StockReserved()
    event.Unpack(e)
    state.on_hand = e.new_on_hand
    state.reserved = e.new_reserved
    state.reservations[e.order_id] = e.quantity


def apply_reservation_released(state: InventoryState, event: AnyProto) -> None:
    e = inventory.ReservationReleased()
    event.Unpack(e)
    state.on_hand = e.new_on_hand
    state.reserved = e.new_reserved
    state.reservations.pop(e.order_id, None)


def apply_reservation_committed(state: InventoryState, event: AnyProto) -> None:
    e = inventory.ReservationCommitted()
    event.Unpack(e)
    state.on_hand = e.new_on_hand
    state.reserved = e.new_reserved
    state.reservations.pop(e.order_id, None)


# ============================================================================
# State rebuilding
# ============================================================================

# stateBuilder is the single source of truth for event type -> applier mapping.
_state_builder = (
    StateBuilder(InventoryState)
    .on("StockInitialized", apply_stock_initialized)
    .on("StockReceived", apply_stock_received)
    .on("StockReserved", apply_stock_reserved)
    .on("ReservationReleased", apply_reservation_released)
    .on("ReservationCommitted", apply_reservation_committed)
)


def rebuild_state(event_book: types.EventBook | None) -> InventoryState:
    """Rebuild inventory state from events."""
    return _state_builder.rebuild(event_book)


def apply_event(state: InventoryState, event: AnyProto) -> None:
    """Apply a single event to the inventory state."""
    _state_builder.apply(state, event)
