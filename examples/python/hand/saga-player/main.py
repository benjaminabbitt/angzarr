"""Saga: Hand -> Player

Reacts to PotAwarded events from Hand domain.
Sends DepositFunds commands to Player domain.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from angzarr_client import next_sequence
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
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


def prepare_pot_awarded(event: Any, root: types.UUID | None) -> list[types.Cover]:
    """Declare all winners as destinations."""
    pot_awarded = hand.PotAwarded()
    event.Unpack(pot_awarded)

    return [
        types.Cover(
            domain="player",
            root=types.UUID(value=winner.player_root),
        )
        for winner in pot_awarded.winners
    ]


def handle_pot_awarded(
    event: Any,
    root: types.UUID | None,
    correlation_id: str,
    destinations: list[types.EventBook],
) -> list[types.CommandBook]:
    """Translate PotAwarded -> DepositFunds for each winner."""
    pot_awarded = hand.PotAwarded()
    event.Unpack(pot_awarded)

    # Build a map from player root to destination for sequence lookup
    dest_map = {}
    for dest in destinations:
        if dest.HasField("cover") and dest.cover.HasField("root"):
            key = dest.cover.root.value.hex()
            dest_map[key] = dest

    commands = []

    # Create DepositFunds commands for each winner
    for winner in pot_awarded.winners:
        player_key = winner.player_root.hex()

        # Get sequence from destination state
        dest_seq = 0
        if player_key in dest_map:
            dest_seq = next_sequence(dest_map[player_key])

        deposit_funds = player.DepositFunds(
            amount=player.Currency(
                amount=winner.amount,
            ),
        )

        cmd_any = Any()
        cmd_any.Pack(deposit_funds, type_url_prefix="type.googleapis.com/")

        commands.append(
            types.CommandBook(
                cover=types.Cover(
                    domain="player",
                    root=types.UUID(value=winner.player_root),
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
    EventRouter("saga-hand-player")
    .domain("hand")
    .prepare("PotAwarded", prepare_pot_awarded)
    .on("PotAwarded", handle_pot_awarded)
)


if __name__ == "__main__":
    handler = SagaHandler(router)
    run_saga_server("saga-hand-player", "50414", handler, logger=logger)
