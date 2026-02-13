"""Saga that syncs table and hand domains."""

from google.protobuf.any_pb2 import Any

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import table_pb2 as table

from .base import Saga, SagaContext


class TableSyncSaga(Saga):
    """
    Syncs table and hand domains.

    When a hand starts at a table:
    1. Deal cards to all active players
    2. Post blinds

    When a hand completes:
    1. Signal the table that hand has ended
    """

    @property
    def name(self) -> str:
        return "TableSyncSaga"

    @property
    def subscribed_events(self) -> list[str]:
        return ["HandStarted", "HandComplete"]

    def handle(self, context: SagaContext) -> list[types.CommandBook]:
        """Handle table-hand sync events."""
        if context.event_type == "HandStarted":
            return self._handle_hand_started(context)
        elif context.event_type == "HandComplete":
            return self._handle_hand_complete(context)
        return []

    def _handle_hand_started(self, context: SagaContext) -> list[types.CommandBook]:
        """
        When table starts a hand, deal cards in the hand domain.
        """
        commands = []

        for page in context.event_book.pages:
            if "HandStarted" not in page.event.type_url:
                continue

            event = table.HandStarted()
            page.event.Unpack(event)

            # Build DealCards command for hand domain
            players = []
            for player_snapshot in event.active_players:
                players.append(
                    hand.PlayerInHand(
                        player_root=player_snapshot.player_root,
                        position=player_snapshot.position,
                        stack=player_snapshot.stack,
                    )
                )

            cmd = hand.DealCards(
                table_root=context.aggregate_root,
                hand_number=event.hand_number,
                game_variant=event.game_variant,
                dealer_position=event.dealer_position,
                small_blind=event.small_blind,
                big_blind=event.big_blind,
            )
            cmd.players.extend(players)

            cmd_any = Any()
            cmd_any.Pack(cmd, type_url_prefix="type.poker/")

            # Get sequence from destination state
            seq = context.next_sequence_for("hand", event.hand_root)

            commands.append(
                types.CommandBook(
                    cover=types.Cover(
                        root=types.UUID(value=event.hand_root),
                        domain="hand",
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

    def _handle_hand_complete(self, context: SagaContext) -> list[types.CommandBook]:
        """
        When hand completes, update the table.
        """
        commands = []

        for page in context.event_book.pages:
            if "HandComplete" not in page.event.type_url:
                continue

            event = hand.HandComplete()
            page.event.Unpack(event)

            # Build EndHand command for table domain
            results = []
            for winner in event.winners:
                results.append(
                    table.PotResult(
                        winner_root=winner.player_root,
                        amount=winner.amount,
                    )
                )

            cmd = table.EndHand(
                hand_root=context.aggregate_root,
            )
            cmd.results.extend(results)

            cmd_any = Any()
            cmd_any.Pack(cmd, type_url_prefix="type.poker/")

            # Get sequence from destination state
            seq = context.next_sequence_for("table", event.table_root)

            commands.append(
                types.CommandBook(
                    cover=types.Cover(
                        root=types.UUID(value=event.table_root),
                        domain="table",
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
