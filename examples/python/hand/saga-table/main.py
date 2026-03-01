"""Saga: Hand -> Table (OO Pattern)

Reacts to HandComplete events from Hand domain.
Sends EndHand commands to Table domain.

Uses the OO-style implementation with the Saga base class
and @domain, @output_domain, @prepares, and @handles decorators.
"""

import sys
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any

sys.path.insert(0, str(Path(__file__).parent.parent.parent))

from angzarr_client import next_sequence
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
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


@domain("hand")
@output_domain("table")
class HandTableSaga(Saga):
    """Saga that translates HandComplete events to EndHand commands.

    Uses the OO pattern with @domain, @output_domain, @prepares, and @handles decorators.
    """

    name = "saga-hand-table"

    @prepares(hand.HandComplete)
    def prepare_hand_complete(self, event: hand.HandComplete) -> list[types.Cover]:
        """Declare the table aggregate as destination."""
        return [
            types.Cover(
                domain="table",
                root=types.UUID(value=event.table_root),
            )
        ]

    @handles(hand.HandComplete)
    def handle_hand_complete(
        self,
        event: hand.HandComplete,
        destinations: list[types.EventBook],
    ) -> types.CommandBook:
        """Translate HandComplete -> EndHand."""
        # Get next sequence from destination state
        dest_seq = next_sequence(destinations[0]) if destinations else 0

        # Get hand_root from context
        hand_root = self.context.root.value if self.context.root else b""

        # Convert PotWinner to PotResult
        results = [
            table.PotResult(
                winner_root=winner.player_root,
                amount=winner.amount,
                pot_type=winner.pot_type,
                winning_hand=winner.winning_hand,
            )
            for winner in event.winners
        ]

        # Build EndHand command
        end_hand = table.EndHand(
            hand_root=hand_root,
        )
        end_hand.results.extend(results)

        cmd_any = Any()
        cmd_any.Pack(end_hand, type_url_prefix="type.googleapis.com/")

        return types.CommandBook(
            cover=types.Cover(
                domain="table",
                root=types.UUID(value=event.table_root),
                correlation_id=self.context.correlation_id,
            ),
            pages=[
                types.CommandPage(
                    sequence=dest_seq,
                    command=cmd_any,
                )
            ],
        )


if __name__ == "__main__":
    handler = SagaHandler(HandTableSaga)
    run_saga_server("saga-hand-table", "50412", handler, logger=logger)
