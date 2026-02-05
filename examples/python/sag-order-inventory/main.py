"""Inventory Reservation Saga: reserves stock when cart items change.

Handles four cart events:
  ItemAdded        -> ReserveStock (reserve inventory for cart item)
  ItemRemoved      -> ReleaseReservation (release when item removed)
  QuantityUpdated  -> ReleaseReservation + ReserveStock (adjust reservation)
  CartCleared      -> ReleaseReservation per item (release all when cart cleared)

The cart's root ID is used as the order_id for reservations,
allowing inventory to track which cart holds each reservation.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from angzarr import types_pb2 as types
from identity import inventory_product_root, to_proto_bytes
from proto import cart_pb2 as cart
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

SAGA_NAME = "inventory-reservation"
SOURCE_DOMAIN = "cart"
TARGET_DOMAIN = "inventory"


def _product_root_bytes(product_id: str) -> bytes:
    """Deterministic UUID bytes for an inventory product aggregate."""
    return to_proto_bytes(inventory_product_root(product_id))


def handle_item_added(
    event: Any, root: types.UUID | None, correlation_id: str
) -> list[types.CommandBook]:
    """ItemAdded -> ReserveStock for the product."""
    if root is None:
        return []

    added = cart.ItemAdded()
    event.Unpack(added)

    cart_id = root.value.hex()
    target_root = _product_root_bytes(added.product_id)

    cmd = inventory.ReserveStock(quantity=added.quantity, order_id=cart_id)
    cmd_any = Any()
    cmd_any.Pack(cmd, type_url_prefix="type.examples/")

    return [
        types.CommandBook(
            cover=types.Cover(
                domain=TARGET_DOMAIN,
                root=types.UUID(value=target_root),
                correlation_id=correlation_id,
            ),
            pages=[types.CommandPage(command=cmd_any)],
        )
    ]


def handle_item_removed(
    event: Any, root: types.UUID | None, correlation_id: str
) -> list[types.CommandBook]:
    """ItemRemoved -> ReleaseReservation for the product."""
    if root is None:
        return []

    removed = cart.ItemRemoved()
    event.Unpack(removed)

    cart_id = root.value.hex()
    target_root = _product_root_bytes(removed.product_id)

    cmd = inventory.ReleaseReservation(order_id=cart_id)
    cmd_any = Any()
    cmd_any.Pack(cmd, type_url_prefix="type.examples/")

    return [
        types.CommandBook(
            cover=types.Cover(
                domain=TARGET_DOMAIN,
                root=types.UUID(value=target_root),
                correlation_id=correlation_id,
            ),
            pages=[types.CommandPage(command=cmd_any)],
        )
    ]


def handle_quantity_updated(
    event: Any, root: types.UUID | None, correlation_id: str
) -> list[types.CommandBook]:
    """QuantityUpdated -> ReleaseReservation + ReserveStock (release old, reserve new)."""
    if root is None:
        return []

    updated = cart.QuantityUpdated()
    event.Unpack(updated)

    cart_id = root.value.hex()
    target_root = _product_root_bytes(updated.product_id)
    cover = types.Cover(
        domain=TARGET_DOMAIN,
        root=types.UUID(value=target_root),
        correlation_id=correlation_id,
    )

    release_cmd = inventory.ReleaseReservation(order_id=cart_id)
    release_any = Any()
    release_any.Pack(release_cmd, type_url_prefix="type.examples/")

    reserve_cmd = inventory.ReserveStock(quantity=updated.new_quantity, order_id=cart_id)
    reserve_any = Any()
    reserve_any.Pack(reserve_cmd, type_url_prefix="type.examples/")

    return [
        types.CommandBook(
            cover=types.Cover(
                domain=cover.domain,
                root=types.UUID(value=target_root),
                correlation_id=correlation_id,
            ),
            pages=[types.CommandPage(command=release_any)],
        ),
        types.CommandBook(
            cover=types.Cover(
                domain=cover.domain,
                root=types.UUID(value=target_root),
                correlation_id=correlation_id,
            ),
            pages=[types.CommandPage(command=reserve_any)],
        ),
    ]


def handle_cart_cleared(
    event: Any, root: types.UUID | None, correlation_id: str
) -> list[types.CommandBook]:
    """CartCleared -> ReleaseReservation for each item that was in the cart."""
    if root is None:
        return []

    cleared = cart.CartCleared()
    event.Unpack(cleared)

    cart_id = root.value.hex()
    commands = []

    for item in cleared.items:
        target_root = _product_root_bytes(item.product_id)

        cmd = inventory.ReleaseReservation(order_id=cart_id)
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
    .on("ItemAdded", handle_item_added)
    .on("ItemRemoved", handle_item_removed)
    .on("QuantityUpdated", handle_quantity_updated)
    .on("CartCleared", handle_cart_cleared)
)

handler = SagaHandler(router)

if __name__ == "__main__":
    run_saga_server(SAGA_NAME, "50310", handler, logger=logger)
