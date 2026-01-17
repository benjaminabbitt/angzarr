"""Product state reconstruction from event stream."""

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    """Calculate the next sequence number for new events."""
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> domains.ProductState:
    """Rebuild product state from event history."""
    state = domains.ProductState()

    if event_book is None or not event_book.pages:
        return state

    if event_book.snapshot and event_book.snapshot.state:
        state.ParseFromString(event_book.snapshot.state.value)

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("ProductCreated"):
            event = domains.ProductCreated()
            page.event.Unpack(event)
            state.sku = event.sku
            state.name = event.name
            state.description = event.description
            state.price_cents = event.price_cents
            state.status = "active"

        elif page.event.type_url.endswith("ProductUpdated"):
            event = domains.ProductUpdated()
            page.event.Unpack(event)
            state.name = event.name
            state.description = event.description

        elif page.event.type_url.endswith("PriceSet"):
            event = domains.PriceSet()
            page.event.Unpack(event)
            state.price_cents = event.new_price_cents

        elif page.event.type_url.endswith("ProductDiscontinued"):
            state.status = "discontinued"

    return state
