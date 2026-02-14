"""Table bounded context gRPC server."""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))

from angzarr_client import run_aggregate_server
from angzarr_client.protoname import name
from angzarr_client import CommandRouter

from handlers import (
    handle_create_table,
    handle_join_table,
    handle_leave_table,
    handle_start_hand,
    handle_end_hand,
)
from handlers.state import TableState, build_state


def state_from_event_book(event_book):
    """Build state from EventBook - extracts Any-wrapped events and applies them."""
    state = TableState()
    if event_book is None:
        return state
    events = [page.event for page in event_book.pages if page.event]
    return build_state(state, events)
from angzarr_client.proto.examples import table_pb2 as table

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
    CommandRouter("table", state_from_event_book)
    .on(name(table.CreateTable), handle_create_table)
    .on(name(table.JoinTable), handle_join_table)
    .on(name(table.LeaveTable), handle_leave_table)
    .on(name(table.StartHand), handle_start_hand)
    .on(name(table.EndHand), handle_end_hand)
)


if __name__ == "__main__":
    run_aggregate_server("table", "50402", router, logger=logger)
