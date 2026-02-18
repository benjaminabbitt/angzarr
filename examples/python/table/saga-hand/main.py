"""Saga: Table -> Hand

Reacts to HandStarted events from Table domain.
Sends DealCards commands to Hand domain.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from angzarr_client import next_sequence
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand
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


def prepare_hand_started(event: Any, root: types.UUID | None) -> list[types.Cover]:
    """Declare the hand aggregate as destination."""
    hand_started = table.HandStarted()
    event.Unpack(hand_started)

    return [
        types.Cover(
            domain="hand",
            root=types.UUID(value=hand_started.hand_root),
        )
    ]


# docs:start:saga_handler
def handle_hand_started(
    event: Any,
    root: types.UUID | None,
    correlation_id: str,
    destinations: list[types.EventBook],
) -> list[types.CommandBook]:
    """Translate HandStarted -> DealCards."""
    hand_started = table.HandStarted()
    event.Unpack(hand_started)

    # Get next sequence from destination state
    dest_seq = next_sequence(destinations[0]) if destinations else 0

    # Convert SeatSnapshot to PlayerInHand
    players = [
        hand.PlayerInHand(
            player_root=seat.player_root,
            position=seat.position,
            stack=seat.stack,
        )
        for seat in hand_started.active_players
    ]

    # Build DealCards command
    deal_cards = hand.DealCards(
        table_root=hand_started.hand_root,
        hand_number=hand_started.hand_number,
        game_variant=hand_started.game_variant,
        dealer_position=hand_started.dealer_position,
        small_blind=hand_started.small_blind,
        big_blind=hand_started.big_blind,
    )
    deal_cards.players.extend(players)

    cmd_any = Any()
    cmd_any.Pack(deal_cards, type_url_prefix="type.googleapis.com/")

    return [
        types.CommandBook(
            cover=types.Cover(
                domain="hand",
                root=types.UUID(value=hand_started.hand_root),
                correlation_id=correlation_id,
            ),
            pages=[
                types.CommandPage(
                    sequence=dest_seq,
                    command=cmd_any,
                )
            ],
        )
    ]
# docs:end:saga_handler


# docs:start:event_router
router = (
    EventRouter("saga-table-hand", "table")
    .sends("hand", "DealCards")
    .prepare("HandStarted", prepare_hand_started)
    .on("HandStarted", handle_hand_started)
)
# docs:end:event_router


if __name__ == "__main__":
    handler = SagaHandler(router)
    run_saga_server("saga-table-hand", "50411", handler, logger=logger)
