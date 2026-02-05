"""Order bounded context gRPC server."""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from aggregate_handler import run_aggregate_server
from protoname import name
from router import CommandRouter

from handlers import (
    handle_create_order,
    handle_apply_loyalty_discount,
    handle_submit_payment,
    handle_confirm_payment,
    handle_cancel_order,
)
from handlers.state import rebuild_state
from proto import order_pb2 as order

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
    CommandRouter("order", rebuild_state)
    .on(name(order.CreateOrder), handle_create_order)
    .on(name(order.ApplyLoyaltyDiscount), handle_apply_loyalty_discount)
    .on(name(order.SubmitPayment), handle_submit_payment)
    .on(name(order.ConfirmPayment), handle_confirm_payment)
    .on(name(order.CancelOrder), handle_cancel_order)
)


if __name__ == "__main__":
    run_aggregate_server("order", "50303", router, logger=logger)
