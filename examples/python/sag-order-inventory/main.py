"""Order-Inventory Saga: reserves stock when orders are created.

Bridges: order -> inventory
Listens to OrderCreated events and generates ReserveStock commands for each line item.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from angzarr import types_pb2 as types
from identity import inventory_product_root, to_proto_bytes
from proto import order_pb2 as order
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

SAGA_NAME = "sag-order-inventory"
SOURCE_DOMAIN = "order"
TARGET_DOMAIN = "inventory"


def _product_root_bytes(product_id: str) -> bytes:
    """Deterministic UUID bytes for an inventory product aggregate."""
    return to_proto_bytes(inventory_product_root(product_id))


def handle_order_created(
    event: Any, root: types.UUID | None, correlation_id: str
) -> list[types.CommandBook]:
    """OrderCreated -> ReserveStock for each line item."""
    if root is None:
        return []

    created = order.OrderCreated()
    event.Unpack(created)

    order_id = root.value.hex()
    commands = []

    for item in created.items:
        target_root = _product_root_bytes(item.product_id)

        cmd = inventory.ReserveStock(quantity=item.quantity, order_id=order_id)
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
    .on("OrderCreated", handle_order_created)
)

handler = SagaHandler(router)

if __name__ == "__main__":
    run_saga_server(SAGA_NAME, "50310", handler, logger=logger)
