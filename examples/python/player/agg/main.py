"""Player bounded context gRPC server."""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))

from angzarr_client import run_aggregate_server
from angzarr_client.protoname import name
from angzarr_client import CommandRouter

from handlers import (
    handle_register_player,
    handle_deposit_funds,
    handle_withdraw_funds,
    handle_reserve_funds,
    handle_release_funds,
    handle_request_action,
)
from handlers.state import PlayerState, build_state


def state_from_event_book(event_book):
    """Build state from EventBook - extracts Any-wrapped events and applies them."""
    state = PlayerState()
    if event_book is None:
        return state
    events = [page.event for page in event_book.pages if page.event]
    return build_state(state, events)
from angzarr_client.proto.examples import player_pb2 as player

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

# docs:start:command_router
router = (
    CommandRouter("player", state_from_event_book)
    .on(name(player.RegisterPlayer), handle_register_player)
    .on(name(player.DepositFunds), handle_deposit_funds)
    .on(name(player.WithdrawFunds), handle_withdraw_funds)
    .on(name(player.ReserveFunds), handle_reserve_funds)
    .on(name(player.ReleaseFunds), handle_release_funds)
    .on(name(player.RequestAction), handle_request_action)
)
# docs:end:command_router


if __name__ == "__main__":
    run_aggregate_server("player", "50401", router, logger=logger)
