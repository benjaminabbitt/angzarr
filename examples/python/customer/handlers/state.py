"""Customer state management.

Contains state dataclasses and state rebuilding logic.
"""

from typing import Protocol

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains


class CustomerState(Protocol):
    """Protocol for customer state."""

    name: str
    email: str
    loyalty_points: int
    lifetime_points: int


class StateRebuildError(Exception):
    """Error during state rebuilding."""


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    """Return the next event sequence number based on prior events."""
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> domains.CustomerState:
    """Rebuild customer state from events."""
    state = domains.CustomerState()

    if event_book is None or not event_book.pages:
        return state

    # Start from snapshot if present
    if event_book.snapshot and event_book.snapshot.state:
        state.ParseFromString(event_book.snapshot.state.value)

    # Apply events
    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("CustomerCreated"):
            event = domains.CustomerCreated()
            page.event.Unpack(event)
            state.name = event.name
            state.email = event.email

        elif page.event.type_url.endswith("LoyaltyPointsAdded"):
            event = domains.LoyaltyPointsAdded()
            page.event.Unpack(event)
            state.loyalty_points = event.new_balance
            state.lifetime_points += event.points

        elif page.event.type_url.endswith("LoyaltyPointsRedeemed"):
            event = domains.LoyaltyPointsRedeemed()
            page.event.Unpack(event)
            state.loyalty_points = event.new_balance

    return state
