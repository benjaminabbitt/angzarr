"""Saga that syncs hand results back to player accounts."""

from google.protobuf.any_pb2 import Any

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import types_pb2 as poker_types

from .base import Saga, SagaContext


class HandResultsSaga(Saga):
    """
    Syncs hand results back to player accounts.

    When a hand completes:
    1. Winners receive their winnings via DepositFunds
    2. Reserved funds are released back to players

    This saga ensures eventual consistency between the hand
    domain and the player domain.
    """

    @property
    def name(self) -> str:
        return "HandResultsSaga"

    @property
    def subscribed_events(self) -> list[str]:
        return ["HandEnded", "PotAwarded"]

    def handle(self, context: SagaContext) -> list[types.CommandBook]:
        """Handle hand result events."""
        if context.event_type == "HandEnded":
            return self._handle_hand_ended(context)
        elif context.event_type == "PotAwarded":
            return self._handle_pot_awarded(context)
        return []

    def _handle_hand_ended(self, context: SagaContext) -> list[types.CommandBook]:
        """
        When hand ends, release reserved funds back to players.

        The HandEnded event from the table domain signals that
        funds should be released.
        """
        commands = []

        for page in context.event_book.pages:
            if "HandEnded" not in page.event.type_url:
                continue

            event = table.HandEnded()
            page.event.Unpack(event)

            # Create ReleaseFunds commands for all players
            for player_hex, amount in event.stack_changes.items():
                player_root = bytes.fromhex(player_hex)

                cmd = player.ReleaseFunds(
                    table_root=event.hand_root,
                )

                cmd_any = Any()
                cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

                # Get sequence from destination state
                seq = context.next_sequence_for("player", player_root)

                commands.append(
                    types.CommandBook(
                        cover=types.Cover(
                            root=types.UUID(value=player_root),
                            domain="player",
                        ),
                        pages=[
                            types.CommandPage(
                                sequence=seq,
                                command=cmd_any,
                            )
                        ],
                    )
                )

        return commands

    def _handle_pot_awarded(self, context: SagaContext) -> list[types.CommandBook]:
        """
        When pot is awarded, transfer funds to winners.

        The PotAwarded event from the hand domain contains
        the final pot distribution.
        """
        commands = []

        for page in context.event_book.pages:
            if "PotAwarded" not in page.event.type_url:
                continue

            event = hand.PotAwarded()
            page.event.Unpack(event)

            # Deposit winnings to each winner
            for winner in event.winners:
                cmd = player.DepositFunds(
                    amount=poker_types.Currency(amount=winner.amount),
                )

                cmd_any = Any()
                cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

                # Get sequence from destination state
                seq = context.next_sequence_for("player", winner.player_root)

                commands.append(
                    types.CommandBook(
                        cover=types.Cover(
                            root=types.UUID(value=winner.player_root),
                            domain="player",
                        ),
                        pages=[
                            types.CommandPage(
                                sequence=seq,
                                command=cmd_any,
                            )
                        ],
                    )
                )

        return commands
