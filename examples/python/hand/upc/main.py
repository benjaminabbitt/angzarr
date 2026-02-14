"""Hand domain upcaster gRPC server.

Uses function-based pattern with UpcasterRouter.
Passthrough upcaster - no transformations yet.
Add @upcaster decorated functions when schema evolution is needed.

Example transformation (when needed):
    @upcaster(CardsDealtV1, CardsDealt)
    def upcast_cards_dealt(old: CardsDealtV1) -> CardsDealt:
        return CardsDealt(
            table_root=old.table_root,
            hand_number=old.hand_number,
            game_variant=GameVariant.TEXAS_HOLDEM,  # New field with default
            ...
        )
    router.on(upcast_cards_dealt)
"""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))

from angzarr_client import UpcasterHandler, UpcasterRouter, run_upcaster_server
from angzarr_client.proto.angzarr import types_pb2 as types


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

router = UpcasterRouter("hand")

# Example (uncomment when needed):
# @upcaster(CardsDealtV1, CardsDealt)
# def upcast_cards_dealt(old: CardsDealtV1) -> CardsDealt:
#     return CardsDealt(...)
# router.on(upcast_cards_dealt)


def handle_upcast(events: list[types.EventPage]) -> list[types.EventPage]:
    """Delegate to router for transformations."""
    return router.upcast(events)


handler = UpcasterHandler("upcaster-hand", "hand").with_handle(handle_upcast)


if __name__ == "__main__":
    run_upcaster_server("upcaster-hand", "50421", handler, logger=logger)
