"""Saga: Table -> Player (OO Pattern)

Reacts to HandEnded events from Table domain.
Sends ReleaseFunds commands to Player domain.

Uses the OO-style implementation with the Saga base class
and @domain, @output_domain, @prepares, and @handles decorators.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from angzarr_client import destination_map, next_sequence
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.saga import Saga, domain, handles, output_domain, prepares
from angzarr_client.saga_handler import SagaHandler, run_saga_server

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


@domain("table")
@output_domain("player")
class TablePlayerSaga(Saga):
    """Saga that translates HandEnded events to ReleaseFunds commands.

    Uses the OO pattern with @domain, @output_domain, @prepares, and @handles decorators.
    This saga produces multiple commands (one per player).
    """

    name = "saga-table-player"

    @prepares(table.HandEnded)
    def prepare_hand_ended(self, event: table.HandEnded) -> list[types.Cover]:
        """Declare all players in StackChanges as destinations."""
        covers = []
        for player_hex in event.stack_changes:
            player_root = bytes.fromhex(player_hex)
            covers.append(
                types.Cover(
                    domain="player",
                    root=types.UUID(value=player_root),
                )
            )
        return covers

    @handles(table.HandEnded)
    def handle_hand_ended(
        self,
        event: table.HandEnded,
        destinations: list[types.EventBook],
    ) -> list[types.CommandBook]:
        """Translate HandEnded -> ReleaseFunds for each player."""
        dest_map = destination_map(destinations)
        commands = []

        # Create ReleaseFunds commands for all players
        for player_hex in event.stack_changes:
            player_root = bytes.fromhex(player_hex)
            dest_seq = next_sequence(dest_map.get(player_hex))

            release_funds = player.ReleaseFunds(
                table_root=event.hand_root,
            )

            cmd_any = Any()
            cmd_any.Pack(release_funds, type_url_prefix="type.googleapis.com/")

            commands.append(
                types.CommandBook(
                    cover=types.Cover(
                        domain="player",
                        root=types.UUID(value=player_root),
                        correlation_id=self.context.correlation_id,
                    ),
                    pages=[
                        types.CommandPage(
                            sequence=dest_seq,
                            command=cmd_any,
                        )
                    ],
                )
            )

        return commands


if __name__ == "__main__":
    handler = SagaHandler(TablePlayerSaga)
    run_saga_server("saga-table-player", "50413", handler, logger=logger)
