"""Player bounded context gRPC server - functional pattern.

This module demonstrates the functional/imperative pattern using:
- CommandRouter for command dispatch
- StateRouter for state reconstruction
- @command_handler decorated functions

Contrasts with the OO pattern in player/agg/ which uses:
- Aggregate[StateT] base class
- @handles and @applies decorators on class methods
"""

import structlog

from angzarr_client import run_aggregate_server, CommandRouter, StateRouter
from angzarr_client.proto.examples import player_pb2 as player

from state import (
    PlayerState,
    apply_registered,
    apply_deposited,
    apply_withdrawn,
    apply_reserved,
    apply_released,
    apply_transferred,
)
from handlers import (
    handle_register,
    handle_deposit,
    handle_withdraw,
    handle_reserve,
    handle_release,
)


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

# State router for event-to-state application
state_router = (
    StateRouter(PlayerState)
    .on(player.PlayerRegistered, apply_registered)
    .on(player.FundsDeposited, apply_deposited)
    .on(player.FundsWithdrawn, apply_withdrawn)
    .on(player.FundsReserved, apply_reserved)
    .on(player.FundsReleased, apply_released)
    .on(player.FundsTransferred, apply_transferred)
)

# Command router with state composition
router = (
    CommandRouter[PlayerState]("player")
    .with_state(state_router)
    .on("RegisterPlayer", handle_register)
    .on("DepositFunds", handle_deposit)
    .on("WithdrawFunds", handle_withdraw)
    .on("ReserveFunds", handle_reserve)
    .on("ReleaseFunds", handle_release)
)


if __name__ == "__main__":
    run_aggregate_server("player", "50402", router, logger=logger)
