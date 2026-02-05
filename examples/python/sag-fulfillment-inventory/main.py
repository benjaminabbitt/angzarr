"""Fulfillment-Inventory Saga: commits inventory reservations when shipments are shipped.

Bridges: fulfillment -> inventory
Listens to Shipped events and generates CommitReservation commands for each line item.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from angzarr import types_pb2 as types
from identity import inventory_product_root, to_proto_bytes
from protoname import name
from proto import fulfillment_pb2 as fulfillment
from proto import inventory_pb2 as inventory
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

SAGA_NAME = "sag-fulfillment-inventory"
SOURCE_DOMAIN = "fulfillment"
TARGET_DOMAIN = "inventory"


def _product_root_bytes(product_id: str) -> bytes:
    """Deterministic UUID bytes for an inventory product aggregate."""
    return to_proto_bytes(inventory_product_root(product_id))


def handle_shipped(
    event: Any, root: types.UUID | None, correlation_id: str
) -> list[types.CommandBook]:
    """Shipped -> CommitReservation for each line item."""
    if root is None:
        return []

    shipped = fulfillment.Shipped()
    event.Unpack(shipped)

    commands = []

    for item in shipped.items:
        target_root = _product_root_bytes(item.product_id)

        cmd = inventory.CommitReservation(order_id=shipped.order_id)
        cmd_any = Any()
        cmd_any.Pack(cmd, type_url_prefix="type.examples/")

        commands.append(
            types.CommandBook(
                cover=types.Cover(
                    domain=TARGET_DOMAIN,
                    root=types.UUID(value=target_root),
                    correlation_id=correlation_id,
                ),
                pages=[types.CommandPage(command=cmd_any)],
            )
        )

    return commands


router = (
    EventRouter(SAGA_NAME, SOURCE_DOMAIN)
    .output(TARGET_DOMAIN)
    .on(name(fulfillment.Shipped), handle_shipped)
)

handler = SagaHandler(router)

if __name__ == "__main__":
    run_saga_server(SAGA_NAME, "50311", handler, logger=logger)
