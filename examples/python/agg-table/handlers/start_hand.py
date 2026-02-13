"""Handler for StartHand command."""

import sys
from pathlib import Path
from datetime import datetime, timezone
import uuid

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import table_pb2 as table

from .state import TableState


def handle_start_hand(
    command_book: types.CommandBook,
    command_any: Any,
    state: TableState,
    seq: int,
) -> types.EventBook:
    """Handle StartHand command."""
    if not state.exists():
        raise CommandRejectedError("Table does not exist")

    if state.status == "in_hand":
        raise CommandRejectedError("Hand already in progress")

    # Need at least 2 active players
    active_count = state.active_player_count()
    if active_count < 2:
        raise CommandRejectedError("Not enough players to start hand")

    # Generate hand root (deterministic from table + hand number)
    hand_number = state.hand_count + 1
    hand_uuid = uuid.uuid5(
        uuid.NAMESPACE_DNS,
        f"angzarr.poker.hand.{state.table_id}.{hand_number}",
    )
    hand_root = hand_uuid.bytes

    # Advance dealer button
    dealer_position = state.next_dealer_position()

    # Get active player positions for blind calculation
    active_positions = sorted(
        pos for pos, seat in state.seats.items() if not seat.is_sitting_out
    )

    # Find small blind and big blind positions
    dealer_idx = 0
    for i, pos in enumerate(active_positions):
        if pos == dealer_position:
            dealer_idx = i
            break

    if len(active_positions) == 2:
        # Heads up: dealer is small blind
        sb_position = active_positions[dealer_idx]
        bb_position = active_positions[(dealer_idx + 1) % 2]
    else:
        sb_position = active_positions[(dealer_idx + 1) % len(active_positions)]
        bb_position = active_positions[(dealer_idx + 2) % len(active_positions)]

    # Build active players list
    active_players = []
    for pos in active_positions:
        seat = state.seats[pos]
        active_players.append(
            table.SeatSnapshot(
                position=pos,
                player_root=seat.player_root,
                stack=seat.stack,
            )
        )

    event = table.HandStarted(
        hand_root=hand_root,
        hand_number=hand_number,
        dealer_position=dealer_position,
        small_blind_position=sb_position,
        big_blind_position=bb_position,
        game_variant=state.game_variant,
        small_blind=state.small_blind,
        big_blind=state.big_blind,
        started_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    event.active_players.extend(active_players)

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.poker/")

    return types.EventBook(
        cover=command_book.cover,
        pages=[
            types.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(
                    seconds=int(datetime.now(timezone.utc).timestamp())
                ),
            )
        ],
    )
