"""Player bounded context gRPC server.

Uses the functional router pattern with @command_handler decorators.
"""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))

from handlers.state import PlayerState, build_state

from angzarr_client import CommandRouter, run_aggregate_server
from angzarr_client.proto.examples import player_pb2 as player
from handlers import (
    handle_deposit_funds,
    handle_register_player,
    handle_release_funds,
    handle_request_action,
    handle_reserve_funds,
    handle_withdraw_funds,
)


def state_from_event_book(event_book):
    """Build state from EventBook - extracts Any-wrapped events and applies them."""
    state = PlayerState()
    if event_book is None:
        return state
    events = [page.event for page in event_book.pages if page.event]
    return build_state(state, events)


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
# Use proto class directly - router extracts full_name (examples.RegisterPlayer)
router = (
    CommandRouter("player", state_from_event_book)
    .on(player.RegisterPlayer, handle_register_player)
    .on(player.DepositFunds, handle_deposit_funds)
    .on(player.WithdrawFunds, handle_withdraw_funds)
    .on(player.ReserveFunds, handle_reserve_funds)
    .on(player.ReleaseFunds, handle_release_funds)
    .on(player.RequestAction, handle_request_action)
)
# docs:end:command_router


if __name__ == "__main__":
    run_aggregate_server(router, "50401", logger=logger)
