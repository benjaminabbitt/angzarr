"""Output projector that renders events as text."""

from typing import Callable

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand

try:
    from .renderer import TextRenderer
except ImportError:
    from renderer import TextRenderer

# Map of type_url suffixes to proto message classes
EVENT_TYPES = {
    # Player events
    "PlayerRegistered": player.PlayerRegistered,
    "FundsDeposited": player.FundsDeposited,
    "FundsWithdrawn": player.FundsWithdrawn,
    "FundsReserved": player.FundsReserved,
    "FundsReleased": player.FundsReleased,
    # Table events
    "TableCreated": table.TableCreated,
    "PlayerJoined": table.PlayerJoined,
    "PlayerLeft": table.PlayerLeft,
    "HandStarted": table.HandStarted,
    "HandEnded": table.HandEnded,
    # Hand events
    "CardsDealt": hand.CardsDealt,
    "BlindPosted": hand.BlindPosted,
    "ActionTaken": hand.ActionTaken,
    "BettingRoundComplete": hand.BettingRoundComplete,
    "CommunityCardsDealt": hand.CommunityCardsDealt,
    "DrawCompleted": hand.DrawCompleted,
    "ShowdownStarted": hand.ShowdownStarted,
    "CardsRevealed": hand.CardsRevealed,
    "CardsMucked": hand.CardsMucked,
    "PotAwarded": hand.PotAwarded,
    "HandComplete": hand.HandComplete,
    "PlayerTimedOut": hand.PlayerTimedOut,
}


# docs:start:projector_functional
class OutputProjector:
    """
    Projector that subscribes to events from all domains and outputs text.

    This is a read-side component that:
    1. Receives events via saga routing
    2. Unpacks them from Any wrappers
    3. Renders them as human-readable text
    4. Outputs to configured destination (console, file, etc.)
    """

    def __init__(
        self,
        output_fn: Callable[[str], None] = print,
        show_timestamps: bool = False,
    ):
        self.renderer = TextRenderer()
        self.output_fn = output_fn
        self.show_timestamps = show_timestamps

    def set_player_name(self, player_root: bytes, name: str):
        """Set display name for a player."""
        self.renderer.set_player_name(player_root, name)

    def handle_event(self, event_page: types.EventPage) -> None:
        """Handle a single event page from any domain."""
        event_any = event_page.event
        type_url = event_any.type_url

        # Extract event type from type_url
        # Format: "type.poker/examples.EventName"
        event_type = type_url.split(".")[-1] if "." in type_url else type_url

        if event_type not in EVENT_TYPES:
            self.output_fn(f"[Unknown event type: {type_url}]")
            return

        # Unpack the event
        event_class = EVENT_TYPES[event_type]
        event = event_class()
        event_any.Unpack(event)

        # Render and output
        text = self.renderer.render(event_type, event)
        if text:
            if self.show_timestamps and event_page.created_at:
                from datetime import datetime, timezone

                ts = datetime.fromtimestamp(event_page.created_at.seconds, tz=timezone.utc)
                text = f"[{ts.strftime('%H:%M:%S')}] {text}"
            self.output_fn(text)

    def handle_event_book(self, event_book: types.EventBook) -> None:
        """Handle all events in an event book."""
        for page in event_book.pages:
            self.handle_event(page)

    def project_from_stream(self, event_stream) -> None:
        """
        Project events from a stream (generator or async iterator).

        This is the main entry point for saga-routed events.
        """
        for event_book in event_stream:
            self.handle_event_book(event_book)
# docs:end:projector_functional
