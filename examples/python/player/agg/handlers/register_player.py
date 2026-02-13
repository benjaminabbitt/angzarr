"""Handler for RegisterPlayer command."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

import sys
from pathlib import Path

# Add paths for imports
sys.path.insert(0, str(Path(__file__).parent.parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client import new_event_book

from .state import PlayerState


def handle_register_player(
    command_book: types.CommandBook,
    command_any: Any,
    state: PlayerState,
    seq: int,
) -> types.EventBook:
    """Handle RegisterPlayer command."""
    if state.exists():
        raise CommandRejectedError("Player already exists")

    cmd = player.RegisterPlayer()
    command_any.Unpack(cmd)

    if not cmd.display_name:
        raise CommandRejectedError("display_name is required")
    if not cmd.email:
        raise CommandRejectedError("email is required")

    event = player.PlayerRegistered(
        display_name=cmd.display_name,
        email=cmd.email,
        player_type=cmd.player_type,
        ai_model_id=cmd.ai_model_id,
        registered_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")

    return new_event_book(command_book, seq, event_any)
