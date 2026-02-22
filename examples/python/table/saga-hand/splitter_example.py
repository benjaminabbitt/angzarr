"""Saga splitter pattern example for documentation.

Demonstrates the splitter pattern where one event triggers commands
to multiple different aggregates.
"""

from angzarr_client import SagaContext
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import player_pb2 as player


# docs:start:saga_splitter
def handle_table_settled(
    event: table.TableSettled, context: SagaContext
) -> list[types.CommandBook]:
    """Split one event into commands for multiple player aggregates."""
    commands = []

    for payout in event.payouts:
        cmd = player.TransferFunds(
            table_root=event.table_root,
            amount=payout.amount,
        )

        target_seq = context.get_sequence("player", payout.player_root)

        commands.append(
            types.CommandBook(
                cover=types.Cover(
                    domain="player", root=types.UUID(value=payout.player_root)
                ),
                pages=[types.CommandPage(sequence=target_seq, command=pack_any(cmd))],
            )
        )

    return commands  # One CommandBook per player


# docs:end:saga_splitter


def pack_any(msg):
    """Helper to pack proto message into Any."""
    from google.protobuf.any_pb2 import Any

    any_msg = Any()
    any_msg.Pack(msg)
    return any_msg
