"""Handler for RevealCards command."""

import sys
from pathlib import Path
from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))
sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import types_pb2 as poker_types

from .state import HandState
from .game_rules import get_game_rules


def handle_reveal_cards(
    command_book: types.CommandBook,
    command_any: Any,
    state: HandState,
    seq: int,
) -> types.EventBook:
    """Handle RevealCards command."""
    if not state.exists():
        raise CommandRejectedError("Hand not dealt")

    if state.status != "showdown":
        raise CommandRejectedError("Not in showdown")

    cmd = hand.RevealCards()
    command_any.Unpack(cmd)

    if not cmd.player_root:
        raise CommandRejectedError("player_root is required")

    # Find the player
    player = None
    for p in state.players.values():
        if p.player_root == cmd.player_root:
            player = p
            break

    if not player:
        raise CommandRejectedError("Player not in hand")

    if player.has_folded:
        raise CommandRejectedError("Player has folded")

    if cmd.muck:
        # Player chooses to muck
        event = hand.CardsMucked(
            player_root=cmd.player_root,
            mucked_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        )
    else:
        # Reveal cards and evaluate hand
        rules = get_game_rules(state.game_variant)
        rank_type, score, kickers = rules.evaluate_hand(
            player.hole_cards,
            state.community_cards,
        )

        event = hand.CardsRevealed(
            player_root=cmd.player_root,
            ranking=poker_types.HandRanking(
                rank_type=rank_type,
                kickers=[k for k in kickers],
                score=score,
            ),
            revealed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        )

        for suit, rank in player.hole_cards:
            event.cards.append(poker_types.Card(suit=suit, rank=rank))

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
