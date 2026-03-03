"""Player bounded context gRPC server - functional pattern.

This module demonstrates the functional/imperative pattern using:
- CommandRouter for command dispatch
- StateRouter for state reconstruction
- @command_handler decorated functions

Contrasts with the OO pattern in player/agg/ which uses:
- CommandHandler[StateT] base class
- @handles and @applies decorators on class methods
"""

import structlog
from state import (
    PlayerState,
    apply_deposited,
    apply_registered,
    apply_released,
    apply_reserved,
    apply_transferred,
    apply_withdrawn,
)

from angzarr_client import CommandRouter, StateRouter, run_command_handler_server
from angzarr_client.proto.examples import player_pb2 as player
from handlers import (
    handle_deposit,
    handle_register,
    handle_release,
    handle_reserve,
    handle_withdraw,
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

# docs:start:command_router
# Command router with state composition
router = (
    CommandRouter[PlayerState]("player")
    .with_state(state_router)
    .on(player.RegisterPlayer, handle_register)
    .on(player.DepositFunds, handle_deposit)
    .on(player.WithdrawFunds, handle_withdraw)
    .on(player.ReserveFunds, handle_reserve)
    .on(player.ReleaseFunds, handle_release)
)
# docs:end:command_router


if __name__ == "__main__":
    run_command_handler_server("player", "50402", router, logger=logger)
