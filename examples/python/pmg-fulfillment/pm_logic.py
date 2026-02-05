"""Order Fulfillment Process Manager - fan-in across order, inventory, and fulfillment domains.

Demonstrates the fan-in pattern: action triggers only when ALL THREE domains
have completed their part. A saga cannot handle this because each saga instance
only sees one domain's event, and race conditions prevent reliable "all complete"
detection.

The Process Manager solves this by maintaining event-sourced state tracking
completed prerequisites, serializing concurrent updates via aggregate sequence,
and using a dispatch_issued flag for exactly-once command dispatch.

Subscribed Domains:
  - order: Listens for PaymentSubmitted
  - inventory: Listens for StockReserved
  - fulfillment: Listens for ItemsPacked

When all three events arrive (any order), emits a Ship command to fulfillment.
"""

from __future__ import annotations

import json
import uuid

from google.protobuf.any_pb2 import Any

from angzarr import types_pb2 as types
from proto import fulfillment_pb2 as fulfillment

PM_NAME = "pmg-fulfillment"
PM_DOMAIN = "pmg-fulfillment"

FULFILLMENT_DOMAIN = "fulfillment"

PREREQ_PAYMENT = "payment"
PREREQ_INVENTORY = "inventory"
PREREQ_FULFILLMENT = "fulfillment"

ALL_PREREQUISITES = [PREREQ_PAYMENT, PREREQ_INVENTORY, PREREQ_FULFILLMENT]

_DISPATCHED_MARKER = "__dispatched__"


def _extract_completed(process_state: types.EventBook) -> list[str]:
    """Extract completed prerequisites from process manager state events."""
    completed: list[str] = []
    for page in process_state.pages:
        event = page.event
        if not event.type_url:
            continue
        if event.type_url.endswith("PrerequisiteCompleted"):
            data = json.loads(event.value)
            prereq = data["prerequisite"]
            if prereq not in completed:
                completed.append(prereq)
        elif event.type_url.endswith("DispatchIssued"):
            completed.append(_DISPATCHED_MARKER)
    return completed


def _classify_event(event: Any) -> str | None:
    """Classify a trigger event into a prerequisite name."""
    if event.type_url.endswith("PaymentSubmitted"):
        return PREREQ_PAYMENT
    if event.type_url.endswith("StockReserved"):
        return PREREQ_INVENTORY
    if event.type_url.endswith("ItemsPacked"):
        return PREREQ_FULFILLMENT
    return None


def _all_complete(completed: list[str]) -> bool:
    """Check if all prerequisites are met."""
    return all(p in completed for p in ALL_PREREQUISITES)


def _already_dispatched(completed: list[str]) -> bool:
    """Check if dispatch was already issued (idempotency)."""
    return _DISPATCHED_MARKER in completed


def _root_id_as_string(root: types.UUID | None) -> str:
    """Convert a proto UUID to a hex string for display."""
    if root is None or not root.value:
        return ""
    return uuid.UUID(bytes=bytes(root.value)).hex


def handle(
    trigger: types.EventBook,
    process_state: types.EventBook,
    destinations: list[types.EventBook],
) -> tuple[list[types.CommandBook], types.EventBook | None]:
    """Handle a trigger event, returning commands and optional PM events.

    Tracks three prerequisites across domains. When all are met, issues
    a Ship command to the fulfillment domain.
    """
    correlation_id = trigger.cover.correlation_id if trigger.cover else ""
    if not correlation_id:
        return ([], None)

    # Get current completed prerequisites from PM state
    completed = _extract_completed(process_state)

    # Already dispatched - idempotent no-op
    if _already_dispatched(completed):
        return ([], None)

    # Classify the trigger event
    new_prerequisite = None
    for page in trigger.pages:
        event = page.event
        if not event.type_url:
            continue
        prereq = _classify_event(event)
        if prereq is not None and prereq not in completed:
            completed.append(prereq)
            new_prerequisite = prereq

    # No new prerequisite from this event
    if new_prerequisite is None:
        return ([], None)

    # Derive deterministic PM root from correlation_id
    pm_root = types.UUID(
        value=uuid.uuid5(uuid.NAMESPACE_OID, correlation_id).bytes,
    )

    # Compute next sequence from existing state
    next_seq = 0
    if process_state.pages:
        last_page = process_state.pages[-1]
        if last_page.HasField("sequence") and last_page.WhichOneof("sequence") == "num":
            next_seq = last_page.num + 1

    pm_pages: list[types.EventPage] = []
    commands: list[types.CommandBook] = []

    # Record prerequisite completion
    remaining = [p for p in ALL_PREREQUISITES if p not in completed]
    prereq_data = json.dumps({
        "prerequisite": new_prerequisite,
        "completed": list(completed),
        "remaining": remaining,
    }).encode()

    pm_pages.append(types.EventPage(
        num=next_seq,
        event=Any(
            type_url="type.examples/examples.PrerequisiteCompleted",
            value=prereq_data,
        ),
    ))

    # Check if all prerequisites met
    if _all_complete(completed):
        dispatch_data = json.dumps({
            "completed": list(completed),
        }).encode()

        pm_pages.append(types.EventPage(
            num=next_seq + 1,
            event=Any(
                type_url="type.examples/examples.DispatchIssued",
                value=dispatch_data,
            ),
        ))

        # Emit Ship command to fulfillment domain
        order_id = _root_id_as_string(
            trigger.cover.root if trigger.cover else None,
        )
        ship_cmd = fulfillment.Ship(
            carrier=f"auto-{order_id}",
            tracking_number="",
        )
        cmd_any = Any()
        cmd_any.Pack(ship_cmd, type_url_prefix="type.examples/")

        commands.append(types.CommandBook(
            cover=types.Cover(
                domain=FULFILLMENT_DOMAIN,
                root=trigger.cover.root if trigger.cover else None,
                correlation_id=correlation_id,
            ),
            pages=[types.CommandPage(command=cmd_any)],
        ))

    pm_event_book = types.EventBook(
        cover=types.Cover(
            domain=PM_DOMAIN,
            root=pm_root,
            correlation_id=correlation_id,
        ),
        pages=pm_pages,
    )

    return (commands, pm_event_book)
