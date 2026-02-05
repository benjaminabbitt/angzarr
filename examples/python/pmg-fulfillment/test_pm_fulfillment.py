"""Tests for the order fulfillment process manager logic."""

from __future__ import annotations

import json
import uuid

from google.protobuf.any_pb2 import Any

from angzarr import types_pb2 as types
from proto import fulfillment_pb2 as fulfillment
from proto import inventory_pb2 as inventory
from proto import order_pb2 as order

from pm_logic import (
    FULFILLMENT_DOMAIN,
    PM_DOMAIN,
    PREREQ_FULFILLMENT,
    PREREQ_INVENTORY,
    PREREQ_PAYMENT,
    handle,
)

CORRELATION_ID = "corr-1"
ORDER_ROOT_BYTES = uuid.uuid5(uuid.NAMESPACE_OID, "order-123").bytes


def _make_event_book(
    domain: str,
    event: Any,
    correlation_id: str,
) -> types.EventBook:
    """Build a trigger EventBook with a single event page."""
    return types.EventBook(
        cover=types.Cover(
            domain=domain,
            root=types.UUID(value=ORDER_ROOT_BYTES),
            correlation_id=correlation_id,
        ),
        pages=[types.EventPage(num=0, event=event)],
    )


def _payment_event() -> Any:
    msg = order.PaymentSubmitted(
        payment_method="card",
        amount_cents=5000,
    )
    a = Any()
    a.Pack(msg, type_url_prefix="type.examples/")
    return a


def _stock_event() -> Any:
    msg = inventory.StockReserved(
        quantity=1,
        order_id="order-123",
        new_available=9,
        new_reserved=1,
        new_on_hand=10,
    )
    a = Any()
    a.Pack(msg, type_url_prefix="type.examples/")
    return a


def _items_packed_event() -> Any:
    msg = fulfillment.ItemsPacked(
        packer_id="packer-1",
    )
    a = Any()
    a.Pack(msg, type_url_prefix="type.examples/")
    return a


def _merge_pm_states(
    state1: types.EventBook | None,
    state2: types.EventBook | None,
) -> types.EventBook | None:
    """Merge two PM state EventBooks (simulating persisted state accumulation)."""
    if state1 is not None and state2 is not None:
        merged = types.EventBook()
        merged.CopyFrom(state1)
        for page in state2.pages:
            new_page = merged.pages.add()
            new_page.CopyFrom(page)
        return merged
    if state1 is not None:
        return state1
    return state2


def _empty_state() -> types.EventBook:
    """Return an empty process state EventBook."""
    return types.EventBook()


def test_first_event_no_dispatch():
    """First prerequisite should record state but not dispatch."""
    trigger = _make_event_book("order", _payment_event(), CORRELATION_ID)

    commands, pm_events = handle(trigger, _empty_state(), [])

    assert len(commands) == 0, "Should not dispatch on first event"
    assert pm_events is not None, "Should produce PM events"
    assert len(pm_events.pages) == 1, "One prerequisite completed"


def test_second_event_no_dispatch():
    """Two prerequisites should not yet trigger dispatch."""
    trigger1 = _make_event_book("order", _payment_event(), CORRELATION_ID)
    _, pm_state1 = handle(trigger1, _empty_state(), [])

    trigger2 = _make_event_book("inventory", _stock_event(), CORRELATION_ID)
    commands, pm_events = handle(trigger2, pm_state1, [])

    assert len(commands) == 0, "Should not dispatch on second event"
    assert pm_events is not None


def test_third_event_triggers_dispatch():
    """All three prerequisites met should produce Ship command."""
    trigger1 = _make_event_book("order", _payment_event(), CORRELATION_ID)
    _, pm_state1 = handle(trigger1, _empty_state(), [])

    trigger2 = _make_event_book("inventory", _stock_event(), CORRELATION_ID)
    _, pm_state2 = handle(trigger2, pm_state1, [])

    # Merge state: combine pm_state1 + pm_state2 pages
    merged_state = _merge_pm_states(pm_state1, pm_state2)

    trigger3 = _make_event_book("fulfillment", _items_packed_event(), CORRELATION_ID)
    commands, pm_events = handle(trigger3, merged_state, [])

    assert len(commands) == 1, "Should dispatch Ship command"
    assert commands[0].cover.domain == FULFILLMENT_DOMAIN

    assert pm_events is not None
    assert len(pm_events.pages) == 2, "PrerequisiteCompleted + DispatchIssued"


def test_idempotent_after_dispatch():
    """After dispatch, duplicate events should be no-ops."""
    # Build state that includes all three prerequisites + DispatchIssued
    dispatched_state = types.EventBook(
        cover=types.Cover(
            domain=PM_DOMAIN,
            root=types.UUID(value=ORDER_ROOT_BYTES),
            correlation_id=CORRELATION_ID,
        ),
        pages=[
            types.EventPage(
                num=0,
                event=Any(
                    type_url="type.examples/examples.PrerequisiteCompleted",
                    value=json.dumps({
                        "prerequisite": PREREQ_PAYMENT,
                        "completed": [PREREQ_PAYMENT],
                        "remaining": [PREREQ_INVENTORY, PREREQ_FULFILLMENT],
                    }).encode(),
                ),
            ),
            types.EventPage(
                num=1,
                event=Any(
                    type_url="type.examples/examples.DispatchIssued",
                    value=json.dumps({
                        "completed": [PREREQ_PAYMENT, PREREQ_INVENTORY, PREREQ_FULFILLMENT],
                    }).encode(),
                ),
            ),
        ],
    )

    trigger = _make_event_book("order", _payment_event(), CORRELATION_ID)
    commands, pm_events = handle(trigger, dispatched_state, [])

    assert len(commands) == 0, "Should not dispatch again"
    assert pm_events is None, "Should not produce events"


def test_no_correlation_id_skips():
    """Events without a correlation_id should be ignored."""
    trigger = _make_event_book("order", _payment_event(), "")

    commands, pm_events = handle(trigger, _empty_state(), [])

    assert len(commands) == 0
    assert pm_events is None


def test_duplicate_prerequisite_noop():
    """A duplicate prerequisite event should produce no new state."""
    trigger1 = _make_event_book("order", _payment_event(), CORRELATION_ID)
    _, pm_state1 = handle(trigger1, _empty_state(), [])

    # Send payment again - same prerequisite already recorded
    trigger2 = _make_event_book("order", _payment_event(), CORRELATION_ID)
    commands, pm_events = handle(trigger2, pm_state1, [])

    assert len(commands) == 0, "Duplicate prerequisite should not dispatch"
    assert pm_events is None, "Duplicate prerequisite should produce no events"
