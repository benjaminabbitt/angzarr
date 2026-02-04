"""Inventory bounded context gRPC server."""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from aggregate_handler import run_aggregate_server
from router import CommandRouter

from handlers import (
    handle_initialize_stock,
    handle_receive_stock,
    handle_reserve_stock,
    handle_release_reservation,
    handle_commit_reservation,
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
    CommandRouter("inventory", rebuild_state)
    .on("InitializeStock", handle_initialize_stock)
    .on("ReceiveStock", handle_receive_stock)
    .on("ReserveStock", handle_reserve_stock)
    .on("ReleaseReservation", handle_release_reservation)
    .on("CommitReservation", handle_commit_reservation)
)


if __name__ == "__main__":
    run_aggregate_server("inventory", "50304", router, logger=logger)
