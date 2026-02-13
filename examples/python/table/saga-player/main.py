"""Saga: Table -> Player

Reacts to HandEnded events from Table domain.
Sends ReleaseFunds commands to Player domain.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from angzarr_client import next_sequence
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.router import EventRouter
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


def prepare_hand_ended(event: Any, root: types.UUID | None) -> list[types.Cover]:
    """Declare all players in StackChanges as destinations."""
    hand_ended = table.HandEnded()
    event.Unpack(hand_ended)

    covers = []
    for player_hex in hand_ended.stack_changes:
        player_root = bytes.fromhex(player_hex)
        covers.append(
            types.Cover(
                domain="player",
                root=types.UUID(value=player_root),
            )
        )
    return covers


def handle_hand_ended(
    event: Any,
    root: types.UUID | None,
    correlation_id: str,
    destinations: list[types.EventBook],
) -> list[types.CommandBook]:
    """Translate HandEnded -> ReleaseFunds for each player."""
    hand_ended = table.HandEnded()
    event.Unpack(hand_ended)

    # Build a map from player root to destination for sequence lookup
    dest_map = {}
    for dest in destinations:
        if dest.HasField("cover") and dest.cover.HasField("root"):
            key = dest.cover.root.value.hex()
            dest_map[key] = dest

    commands = []

    # Create ReleaseFunds commands for all players
    for player_hex in hand_ended.stack_changes:
        player_root = bytes.fromhex(player_hex)

        # Get sequence from destination state
        dest_seq = 0
        if player_hex in dest_map:
            dest_seq = next_sequence(dest_map[player_hex])

        release_funds = player.ReleaseFunds(
            table_root=hand_ended.hand_root,
        )

        cmd_any = Any()
        cmd_any.Pack(release_funds, type_url_prefix="type.googleapis.com/")

        commands.append(
            types.CommandBook(
                cover=types.Cover(
                    domain="player",
                    root=types.UUID(value=player_root),
                    correlation_id=correlation_id,
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


router = (
    EventRouter("saga-table-player", "table")
    .sends("player", "ReleaseFunds")
    .prepare("HandEnded", prepare_hand_ended)
    .on("HandEnded", handle_hand_ended)
)


if __name__ == "__main__":
    handler = SagaHandler(router)
    run_saga_server("saga-table-player", "50413", handler, logger=logger)
