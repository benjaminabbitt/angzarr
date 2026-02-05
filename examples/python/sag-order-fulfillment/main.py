"""Fulfillment Saga: creates shipments when orders complete.

Two-phase protocol:
  Prepare  → declare the fulfillment aggregate as destination
  Execute  → produce CreateShipment commands with target sequence
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from angzarr import types_pb2 as types
from protoname import name
from proto import fulfillment_pb2 as fulfillment
from proto import order_pb2 as order
from router import EventRouter
from saga_handler import SagaHandler, run_saga_server

structlog.configure(
    processors=[
        structlog.stdlib.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.make_filtering_bound_logger(0),
    context_class=dict,
    logger_factory=structlog.PrintLoggerFactory(),
)

logger = structlog.get_logger()

SAGA_NAME = "sag-order-fulfillment"
SOURCE_DOMAIN = "order"
TARGET_DOMAIN = "fulfillment"


def prepare(source: types.EventBook) -> list[types.Cover]:
    """Declare which destination aggregates are needed.

    Returns the fulfillment aggregate cover for optimistic concurrency.
    """
    if not source.pages:
        return []

    if not source.cover.root.value:
        return []

    for page in source.pages:
        if not page.event.type_url.endswith(name(order.OrderCompleted)):
            continue
        return [types.Cover(domain=TARGET_DOMAIN, root=source.cover.root)]

    return []


def execute(
    source: types.EventBook, destinations: list[types.EventBook],
) -> list[types.CommandBook]:
    """Produce commands given source and destination state."""
    if not source.pages:
        return []

    if not source.cover.root.value:
        return []

    target_sequence = 0
    if destinations and destinations[0].pages:
        target_sequence = len(destinations[0].pages)

    order_id = source.cover.root.value.hex()
    commands = []

    for page in source.pages:
        if not page.event.type_url.endswith(name(order.OrderCompleted)):
            continue

        event = order.OrderCompleted()
        page.event.Unpack(event)

        cmd = fulfillment.CreateShipment(order_id=order_id)
        cmd.items.extend(event.items)
        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.examples/")

        cmd_book = types.CommandBook(
            cover=types.Cover(
                domain=TARGET_DOMAIN,
                root=source.cover.root,
                correlation_id=source.cover.correlation_id,
            ),
            pages=[types.CommandPage(sequence=target_sequence, command=cmd_any)],
        )

        commands.append(cmd_book)

    return commands


# Router provides descriptor metadata (listened event types, output domains).
# with_prepare/with_execute override dispatch for two-phase protocol.
router = (
    EventRouter(SAGA_NAME, SOURCE_DOMAIN)
    .output(TARGET_DOMAIN)
    .on(name(order.OrderCompleted), lambda source, event: [])
)

handler = (
    SagaHandler(router)
    .with_prepare(prepare)
    .with_execute(execute)
)

if __name__ == "__main__":
    run_saga_server(SAGA_NAME, "50307", handler, logger=logger)
