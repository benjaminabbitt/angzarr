"""Fulfillment state rebuilding logic."""

from dataclasses import dataclass

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from angzarr.state_builder import StateBuilder
from proto import fulfillment_pb2 as fulfillment


@dataclass
class FulfillmentState:
    order_id: str = ""
    status: str = ""  # "pending", "picking", "packing", "shipped", "delivered"
    tracking_number: str = ""
    carrier: str = ""
    picker_id: str = ""
    packer_id: str = ""
    signature: str = ""
    items: list = None

    def __post_init__(self):
        if self.items is None:
            self.items = []

    def exists(self) -> bool:
        return bool(self.order_id)

    def is_pending(self) -> bool:
        return self.status == "pending"

    def is_picking(self) -> bool:
        return self.status == "picking"

    def is_packing(self) -> bool:
        return self.status == "packing"

    def is_shipped(self) -> bool:
        return self.status == "shipped"

    def is_delivered(self) -> bool:
        return self.status == "delivered"


# ============================================================================
# Named event appliers
# ============================================================================


def apply_shipment_created(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.ShipmentCreated()
    event.Unpack(e)
    state.order_id = e.order_id
    state.status = e.status
    state.items = list(e.items)


def apply_items_picked(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.ItemsPicked()
    event.Unpack(e)
    state.picker_id = e.picker_id
    state.status = "picking"


def apply_items_packed(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.ItemsPacked()
    event.Unpack(e)
    state.packer_id = e.packer_id
    state.status = "packing"


def apply_shipped(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.Shipped()
    event.Unpack(e)
    state.carrier = e.carrier
    state.tracking_number = e.tracking_number
    state.status = "shipped"


def apply_delivered(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.Delivered()
    event.Unpack(e)
    state.signature = e.signature
    state.status = "delivered"


# ============================================================================
# State rebuilding
# ============================================================================

# stateBuilder is the single source of truth for event type -> applier mapping.
_state_builder = (
    StateBuilder(FulfillmentState)
    .on("ShipmentCreated", apply_shipment_created)
    .on("ItemsPicked", apply_items_picked)
    .on("ItemsPacked", apply_items_packed)
    .on("Shipped", apply_shipped)
    .on("Delivered", apply_delivered)
)


def rebuild_state(event_book: types.EventBook | None) -> FulfillmentState:
    """Rebuild fulfillment state from events."""
    return _state_builder.rebuild(event_book)


def apply_event(state: FulfillmentState, event: AnyProto) -> None:
    """Apply a single event to the fulfillment state."""
    _state_builder.apply(state, event)
