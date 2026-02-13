"""Handler for DealCommunityCards command."""

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


def handle_deal_community_cards(
    command_book: types.CommandBook,
    command_any: Any,
    state: HandState,
    seq: int,
) -> types.EventBook:
    """Handle DealCommunityCards command."""
    if not state.exists():
        raise CommandRejectedError("Hand not dealt")

    if state.status == "complete":
        raise CommandRejectedError("Hand is complete")

    cmd = hand.DealCommunityCards()
    command_any.Unpack(cmd)

    if cmd.count <= 0:
        raise CommandRejectedError("Must deal at least 1 card")

    # Get game rules
    rules = get_game_rules(state.game_variant)

    # Check if this variant supports community cards
    if rules.variant == poker_types.FIVE_CARD_DRAW:
        raise CommandRejectedError("Five card draw doesn't have community cards")

    # Determine next phase
    transition = rules.get_next_phase(state.current_phase)
    if not transition:
        raise CommandRejectedError("No more phases")

    if transition.community_cards_to_deal != cmd.count:
        raise CommandRejectedError(
            f"Expected {transition.community_cards_to_deal} cards for this phase"
        )

    # Deal cards from remaining deck
    if len(state.remaining_deck) < cmd.count:
        raise CommandRejectedError("Not enough cards in deck")

    new_cards = state.remaining_deck[: cmd.count]
    all_community = state.community_cards + new_cards

    # Build event
    event = hand.CommunityCardsDealt(
        phase=transition.next_phase,
        dealt_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    for suit, rank in new_cards:
        event.cards.append(poker_types.Card(suit=suit, rank=rank))

    for suit, rank in all_community:
        event.all_community_cards.append(poker_types.Card(suit=suit, rank=rank))

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
