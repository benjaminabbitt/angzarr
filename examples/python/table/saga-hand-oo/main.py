"""Saga: Table -> Hand (OO Pattern)

Reacts to HandStarted events from Table domain.
Sends DealCards commands to Hand domain.

This is the OO-style implementation using the Saga base class
with @prepares and @reacts_to decorators.
"""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from angzarr_client import Saga, prepares, reacts_to, next_sequence
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand
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


class TableHandSaga(Saga):
    """Saga that translates HandStarted events to DealCards commands.

    Uses the OO pattern with @prepares and @reacts_to decorators.
    """

    name = "saga-table-hand"
    input_domain = "table"
    output_domain = "hand"

    @prepares(table.HandStarted)
    def prepare_hand_started(self, event: table.HandStarted) -> list[types.Cover]:
        """Declare the hand aggregate as destination."""
        return [
            types.Cover(
                domain="hand",
                root=types.UUID(value=event.hand_root),
            )
        ]

    @reacts_to(table.HandStarted)
    def handle_hand_started(
        self,
        event: table.HandStarted,
        destinations: list[types.EventBook],
    ) -> types.CommandBook:
        """Translate HandStarted -> DealCards."""
        # Get next sequence from destination state
        dest_seq = next_sequence(destinations[0]) if destinations else 0

        # Convert SeatSnapshot to PlayerInHand
        players = [
            hand.PlayerInHand(
                player_root=seat.player_root,
                position=seat.position,
                stack=seat.stack,
            )
            for seat in event.active_players
        ]

        # Build DealCards command
        deal_cards = hand.DealCards(
            table_root=event.hand_root,
            hand_number=event.hand_number,
            game_variant=event.game_variant,
            dealer_position=event.dealer_position,
            small_blind=event.small_blind,
            big_blind=event.big_blind,
        )
        deal_cards.players.extend(players)

        # Return pre-packed CommandBook for full control
        from google.protobuf.any_pb2 import Any

        cmd_any = Any()
        cmd_any.Pack(deal_cards, type_url_prefix="type.googleapis.com/")

        return types.CommandBook(
            cover=types.Cover(
                domain="hand",
                root=types.UUID(value=event.hand_root),
            ),
            pages=[
                types.CommandPage(
                    sequence=dest_seq,
                    command=cmd_any,
                )
            ],
        )


if __name__ == "__main__":
    handler = SagaHandler(TableHandSaga)
    run_saga_server("saga-table-hand", "50411", handler, logger=logger)
