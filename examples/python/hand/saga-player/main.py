"""Saga: Hand -> Player (OO Pattern)

Reacts to PotAwarded events from Hand domain.
Sends DepositFunds commands to Player domain.

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
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import player_pb2 as player
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
@output_domain("player")
class HandPlayerSaga(Saga):
    """Saga that translates PotAwarded events to DepositFunds commands.

    Uses the OO pattern with @domain, @output_domain, @prepares, and @handles decorators.
    This saga produces multiple commands (one per winner).
    """

    name = "saga-hand-player"

    @prepares(hand.PotAwarded)
    def prepare_pot_awarded(self, event: hand.PotAwarded) -> list[types.Cover]:
        """Declare all winners as destinations."""
        return [
            types.Cover(
                domain="player",
                root=types.UUID(value=winner.player_root),
            )
            for winner in event.winners
        ]

    @handles(hand.PotAwarded)
    def handle_pot_awarded(
        self,
        event: hand.PotAwarded,
        destinations: list[types.EventBook],
    ) -> list[types.CommandBook]:
        """Translate PotAwarded -> DepositFunds for each winner."""
        dest_map = destination_map(destinations)
        commands = []

        # Create DepositFunds commands for each winner
        for winner in event.winners:
            player_key = winner.player_root.hex()
            dest_seq = next_sequence(dest_map.get(player_key))

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
    handler = SagaHandler(HandPlayerSaga)
    run_saga_server("saga-hand-player", "50414", handler, logger=logger)
