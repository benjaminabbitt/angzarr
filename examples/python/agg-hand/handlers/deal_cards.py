"""Handler for DealCards command."""

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


def handle_deal_cards(
    command_book: types.CommandBook,
    command_any: Any,
    state: HandState,
    seq: int,
) -> types.EventBook:
    """Handle DealCards command."""
    if state.exists():
        raise CommandRejectedError("Hand already dealt")

    cmd = hand.DealCards()
    command_any.Unpack(cmd)

    if not cmd.players:
        raise CommandRejectedError("No players in hand")

    if len(cmd.players) < 2:
        raise CommandRejectedError("Need at least 2 players")

    # Get game rules for this variant
    rules = get_game_rules(cmd.game_variant)

    # Deal cards
    player_roots = [p.player_root for p in cmd.players]
    deal_result = rules.deal_hole_cards(
        deck=[],
        players=player_roots,
        seed=cmd.deck_seed if cmd.deck_seed else None,
    )

    # Build player cards
    player_cards = []
    for player_root, cards in deal_result.player_cards.items():
        pc = hand.PlayerHoleCards(player_root=player_root)
        for suit, rank in cards:
            pc.cards.append(poker_types.Card(suit=suit, rank=rank))
        player_cards.append(pc)

    # Build event
    event = hand.CardsDealt(
        table_root=cmd.table_root,
        hand_number=cmd.hand_number,
        game_variant=cmd.game_variant,
        dealer_position=cmd.dealer_position,
        dealt_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    event.player_cards.extend(player_cards)
    event.players.extend(cmd.players)

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
