"""Saga that routes all events to the output projector."""

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent.parent))

from angzarr_client.proto.angzarr import types_pb2 as types

from .base import Saga, SagaContext


class OutputSaga(Saga):
    """
    Routes all events to the output projector for text rendering.

    This saga subscribes to every event type and forwards them
    to the OutputProjector which renders them as human-readable text.
    """

    # All event types we want to render
    ALL_EVENTS = [
        # Player events
        "PlayerRegistered",
        "FundsDeposited",
        "FundsWithdrawn",
        "FundsReserved",
        "FundsReleased",
        "FundsTransferred",
        "ActionRequested",
        # Table events
        "TableCreated",
        "PlayerJoined",
        "PlayerLeft",
        "HandStarted",
        "HandEnded",
        # Hand events
        "CardsDealt",
        "BlindPosted",
        "ActionTaken",
        "BettingRoundComplete",
        "CommunityCardsDealt",
        "DrawCompleted",
        "ShowdownStarted",
        "CardsRevealed",
        "CardsMucked",
        "PotAwarded",
        "HandComplete",
        "PlayerTimedOut",
    ]

    def __init__(self, projector):
        """Initialize with an output projector instance."""
        self.projector = projector

    @property
    def name(self) -> str:
        return "OutputSaga"

    @property
    def subscribed_events(self) -> list[str]:
        return self.ALL_EVENTS

    def handle(self, context: SagaContext) -> list[types.CommandBook]:
        """Forward events to output projector."""
        self.projector.handle_event_book(context.event_book)
        # Output saga doesn't emit commands
        return []
