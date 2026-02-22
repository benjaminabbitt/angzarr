"""Player domain upcaster gRPC server.

Uses OO pattern with Upcaster ABC.
Passthrough upcaster - no transformations yet.
Add @upcasts methods when schema evolution is needed.

Example transformation (when needed):
    @upcasts(PlayerRegisteredV1, PlayerRegistered)
    def upcast_registered(self, old: PlayerRegisteredV1) -> PlayerRegistered:
        return PlayerRegistered(
            display_name=old.display_name,
            email=old.email,
            player_type=old.player_type,
            ai_model_id="",  # New field with default
            registered_at=timestamp_pb2.Timestamp(),
        )
"""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))

from angzarr_client import Upcaster, UpcasterHandler, run_upcaster_server


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


class PlayerUpcaster(Upcaster):
    """Player domain upcaster.

    Add @upcasts decorated methods here when schema evolution is needed.
    Events without matching handlers pass through unchanged.
    """

    name = "upcaster-player"
    domain = "player"

    # Example (uncomment when needed):
    # @upcasts(PlayerRegisteredV1, PlayerRegistered)
    # def upcast_registered(self, old: PlayerRegisteredV1) -> PlayerRegistered:
    #     return PlayerRegistered(...)


handler = UpcasterHandler("upcaster-player", "player").with_handle(
    PlayerUpcaster.handle
)


if __name__ == "__main__":
    run_upcaster_server("upcaster-player", "50411", handler, logger=logger)
