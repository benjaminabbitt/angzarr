"""Fulfillment bounded context gRPC server."""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from aggregate_handler import run_aggregate_server
from protoname import name
from router import CommandRouter

from handlers import (
    handle_create_shipment,
    handle_mark_picked,
    handle_mark_packed,
    handle_ship,
    handle_record_delivery,
)
from handlers.state import rebuild_state
from proto import fulfillment_pb2 as fulfillment

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
    .on(name(fulfillment.CreateShipment), handle_create_shipment)
    .on(name(fulfillment.MarkPicked), handle_mark_picked)
    .on(name(fulfillment.MarkPacked), handle_mark_packed)
    .on(name(fulfillment.Ship), handle_ship)
    .on(name(fulfillment.RecordDelivery), handle_record_delivery)
)


if __name__ == "__main__":
    run_aggregate_server("fulfillment", "50305", router, logger=logger)
