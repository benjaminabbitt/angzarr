"""Fulfillment bounded context gRPC server."""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from aggregate_handler import run_aggregate_server
from router import CommandRouter

from handlers import (
    handle_create_shipment,
    handle_mark_picked,
    handle_mark_packed,
    handle_ship,
    handle_record_delivery,
)
from handlers.state import rebuild_state

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

router = (
    CommandRouter("fulfillment", rebuild_state)
    .on("CreateShipment", handle_create_shipment)
    .on("MarkPicked", handle_mark_picked)
    .on("MarkPacked", handle_mark_packed)
    .on("Ship", handle_ship)
    .on("RecordDelivery", handle_record_delivery)
)


if __name__ == "__main__":
    run_aggregate_server("fulfillment", "50305", router, logger=logger)
