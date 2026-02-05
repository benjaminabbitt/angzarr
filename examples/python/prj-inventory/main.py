"""Inventory Projector: logs inventory events for read model.

Consumes inventory domain events and logs stock level changes.
Demonstrates the projector pattern for building read models.
"""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from angzarr import types_pb2 as types
from projector_handler import ProjectorHandler, run_projector_server
from proto import inventory_pb2 as inventory

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

PROJECTOR_NAME = "prj-inventory"
SOURCE_DOMAIN = "inventory"


def handle(book: types.EventBook) -> types.Projection:
    """Process inventory events and log them."""
    for page in book.pages:
        if not page.event.type_url:
            continue
        process_event(page.event)
    return types.Projection()


def process_event(event) -> None:
    """Process a single event and log it."""
    type_url = event.type_url

    if type_url.endswith("StockInitialized"):
        e = inventory.StockInitialized()
        event.Unpack(e)
        logger.info(
            "inventory_projected",
            event="StockInitialized",
            product_id=e.product_id,
            quantity=e.quantity,
            threshold=e.low_stock_threshold,
        )
    elif type_url.endswith("StockReceived"):
        e = inventory.StockReceived()
        event.Unpack(e)
        logger.info(
            "inventory_projected",
            event="StockReceived",
            quantity=e.quantity,
            new_on_hand=e.new_on_hand,
            reference=e.reference,
        )
    elif type_url.endswith("StockReserved"):
        e = inventory.StockReserved()
        event.Unpack(e)
        logger.info(
            "inventory_projected",
            event="StockReserved",
            order_id=e.order_id,
            quantity=e.quantity,
            new_available=e.new_available,
            new_reserved=e.new_reserved,
        )
    elif type_url.endswith("ReservationReleased"):
        e = inventory.ReservationReleased()
        event.Unpack(e)
        logger.info(
            "inventory_projected",
            event="ReservationReleased",
            order_id=e.order_id,
            quantity=e.quantity,
            new_available=e.new_available,
        )
    elif type_url.endswith("ReservationCommitted"):
        e = inventory.ReservationCommitted()
        event.Unpack(e)
        logger.info(
            "inventory_projected",
            event="ReservationCommitted",
            order_id=e.order_id,
            quantity=e.quantity,
            new_on_hand=e.new_on_hand,
        )
    elif type_url.endswith("LowStockAlert"):
        e = inventory.LowStockAlert()
        event.Unpack(e)
        logger.info(
            "inventory_projected",
            event="LowStockAlert",
            product_id=e.product_id,
            available=e.available,
            threshold=e.threshold,
        )


handler = ProjectorHandler(PROJECTOR_NAME, SOURCE_DOMAIN).with_handle(handle)

if __name__ == "__main__":
    run_projector_server(PROJECTOR_NAME, "50360", handler, logger=logger)
