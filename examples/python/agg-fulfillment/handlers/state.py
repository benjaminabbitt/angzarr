"""Fulfillment state rebuilding logic."""

from dataclasses import dataclass

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from protoname import name
from state_builder import StateBuilder
from proto import fulfillment_pb2 as fulfillment

from .status import FulfillmentStatus


@dataclass
class FulfillmentState:
    order_id: str = ""
    status: FulfillmentStatus | str = ""
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
        return self.status == FulfillmentStatus.PENDING

    def is_picking(self) -> bool:
        return self.status == FulfillmentStatus.PICKING

    def is_packing(self) -> bool:
        return self.status == FulfillmentStatus.PACKING

    def is_shipped(self) -> bool:
        return self.status == FulfillmentStatus.SHIPPED

    def is_delivered(self) -> bool:
        return self.status == FulfillmentStatus.DELIVERED


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
    state.status = FulfillmentStatus.PICKING


def apply_items_packed(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.ItemsPacked()
    event.Unpack(e)
    state.packer_id = e.packer_id
    state.status = FulfillmentStatus.PACKING


def apply_shipped(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.Shipped()
    event.Unpack(e)
    state.carrier = e.carrier
    state.tracking_number = e.tracking_number
    state.status = FulfillmentStatus.SHIPPED


def apply_delivered(state: FulfillmentState, event: AnyProto) -> None:
    e = fulfillment.Delivered()
    event.Unpack(e)
    state.signature = e.signature
    state.status = FulfillmentStatus.DELIVERED


# ============================================================================
# State rebuilding
# ============================================================================

# stateBuilder is the single source of truth for event type -> applier mapping.
_state_builder = (
    StateBuilder(FulfillmentState)
    .on(name(fulfillment.ShipmentCreated), apply_shipment_created)
    .on(name(fulfillment.ItemsPicked), apply_items_picked)
    .on(name(fulfillment.ItemsPacked), apply_items_packed)
    .on(name(fulfillment.Shipped), apply_shipped)
    .on(name(fulfillment.Delivered), apply_delivered)
)


def rebuild_state(event_book: types.EventBook | None) -> FulfillmentState:
    """Rebuild fulfillment state from events."""
    return _state_builder.rebuild(event_book)


def apply_event(state: FulfillmentState, event: AnyProto) -> None:
    """Apply a single event to the fulfillment state."""
    _state_builder.apply(state, event)
