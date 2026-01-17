"""Fulfillment state management and event sourcing."""

from dataclasses import dataclass

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains


@dataclass
class FulfillmentState:
    order_id: str = ""
    status: str = ""  # "pending", "picking", "packing", "shipped", "delivered"
    tracking_number: str = ""
    carrier: str = ""
    picker_id: str = ""
    packer_id: str = ""
    signature: str = ""

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


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> FulfillmentState:
    state = FulfillmentState()

    if event_book is None or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("ShipmentCreated"):
            event = domains.ShipmentCreated()
            page.event.Unpack(event)
            state.order_id = event.order_id
            state.status = event.status

        elif page.event.type_url.endswith("ItemsPicked"):
            event = domains.ItemsPicked()
            page.event.Unpack(event)
            state.picker_id = event.picker_id
            state.status = "picking"

        elif page.event.type_url.endswith("ItemsPacked"):
            event = domains.ItemsPacked()
            page.event.Unpack(event)
            state.packer_id = event.packer_id
            state.status = "packing"

        elif page.event.type_url.endswith("Shipped"):
            event = domains.Shipped()
            page.event.Unpack(event)
            state.carrier = event.carrier
            state.tracking_number = event.tracking_number
            state.status = "shipped"

        elif page.event.type_url.endswith("Delivered"):
            event = domains.Delivered()
            page.event.Unpack(event)
            state.signature = event.signature
            state.status = "delivered"

    return state
