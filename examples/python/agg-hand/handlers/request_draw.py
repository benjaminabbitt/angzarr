"""Handler for RequestDraw command."""

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


def handle_request_draw(
    command_book: types.CommandBook,
    command_any: Any,
    state: HandState,
    seq: int,
) -> types.EventBook:
    """Handle RequestDraw command."""
    if not state.exists():
        raise CommandRejectedError("Hand not dealt")

    if state.status == "complete":
        raise CommandRejectedError("Hand is complete")

    cmd = hand.RequestDraw()
    command_any.Unpack(cmd)

    if not cmd.player_root:
        raise CommandRejectedError("player_root is required")

    # Check game variant supports draw
    rules = get_game_rules(state.game_variant)
    if rules.variant != poker_types.FIVE_CARD_DRAW:
        raise CommandRejectedError("Draw not supported for this game variant")

    if state.current_phase != poker_types.DRAW:
        raise CommandRejectedError("Not in draw phase")

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

    # Validate card indices
    if len(cmd.card_indices) > 5:
        raise CommandRejectedError("Cannot discard more than 5 cards")

    for idx in cmd.card_indices:
        if idx < 0 or idx >= len(player.hole_cards):
            raise CommandRejectedError(f"Invalid card index: {idx}")

    # Check deck has enough cards
    if len(state.remaining_deck) < len(cmd.card_indices):
        raise CommandRejectedError("Not enough cards in deck")

    # Calculate new cards
    cards_discarded = len(cmd.card_indices)
    keep_indices = [
        i for i in range(len(player.hole_cards)) if i not in cmd.card_indices
    ]
    kept_cards = [player.hole_cards[i] for i in keep_indices]
    new_cards = state.remaining_deck[:cards_discarded]
    final_hand = kept_cards + new_cards

    event = hand.DrawCompleted(
        player_root=cmd.player_root,
        cards_discarded=cards_discarded,
        cards_drawn=cards_discarded,
        drawn_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    for suit, rank in new_cards:
        event.new_cards.append(poker_types.Card(suit=suit, rank=rank))

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
