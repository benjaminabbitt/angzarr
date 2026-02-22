"""Hand bounded context gRPC server."""

import sys
from pathlib import Path

import structlog

sys.path.insert(0, str(Path(__file__).parent.parent.parent / "angzarr"))

from angzarr_client import run_aggregate_server
from angzarr_client.protoname import name
from angzarr_client import CommandRouter

from handlers import (
    handle_deal_cards,
    handle_post_blind,
    handle_player_action,
    handle_deal_community_cards,
    handle_request_draw,
    handle_reveal_cards,
    handle_award_pot,
)
from handlers.state import HandState, build_state


def state_from_event_book(event_book):
    """Build state from EventBook - extracts Any-wrapped events and applies them."""
    state = HandState()
    if event_book is None or not event_book.pages:
        return state

    # Sort by sequence for correct ordering
    def get_seq(p):
        if p.WhichOneof("sequence") == "num":
            return p.num
        return 0

    sorted_pages = sorted(event_book.pages, key=get_seq)
    events = [page.event for page in sorted_pages if page.event]
    return build_state(state, events)


from angzarr_client.proto.examples import hand_pb2 as hand

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

router = (
    CommandRouter("hand", state_from_event_book)
    .on(name(hand.DealCards), handle_deal_cards)
    .on(name(hand.PostBlind), handle_post_blind)
    .on(name(hand.PlayerAction), handle_player_action)
    .on(name(hand.DealCommunityCards), handle_deal_community_cards)
    .on(name(hand.RequestDraw), handle_request_draw)
    .on(name(hand.RevealCards), handle_reveal_cards)
    .on(name(hand.AwardPot), handle_award_pot)
)


if __name__ == "__main__":
    run_aggregate_server("hand", "50403", router, logger=logger)
