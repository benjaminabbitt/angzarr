"""Order Fulfillment Process Manager gRPC server.

Fan-in process manager tracking payment, inventory, and fulfillment prerequisites.
When all three are complete, issues a Ship command to the fulfillment domain.
"""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent / "angzarr"))

from process_manager_handler import ProcessManagerHandler, run_process_manager_server
from pm_logic import PM_NAME, handle

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

handler = (
    ProcessManagerHandler(PM_NAME)
    .listen_to("order")
    .listen_to("inventory")
    .listen_to("fulfillment")
    .with_handle(handle)
)

if __name__ == "__main__":
    run_process_manager_server(PM_NAME, "50320", handler, logger=logger)
