"""Scenario context for BDD-style testing.

Provides a context object for tracking state across test steps,
particularly useful for Gherkin/BDD scenarios.
"""

from dataclasses import dataclass, field
from typing import Any

from ..errors import CommandRejectedError
from ..helpers import now
from ..proto.angzarr import UUID, Cover, EventBook, EventPage
from .builders import pack_event


@dataclass
class ScenarioContext:
    """Shared context for BDD test scenarios.

    Tracks the current aggregate, event history, command results,
    and rebuilt state across Given/When/Then steps.

    Attributes:
        domain: Current aggregate domain being tested
        root: Current aggregate root as bytes
        events: List of packed events (ProtoAny) in history
        result: Last command handler result (event or tuple of events)
        error: Last CommandRejectedError if command was rejected
        state: Rebuilt aggregate state after applying events

    Example:
        ctx = ScenarioContext()
        ctx.domain = "player"
        ctx.root = uuid_for("player-alice")

        # Given player registered
        ctx.add_event(PlayerRegistered(email="alice@test.com"))

        # When deposit funds
        try:
            ctx.result = handler.handle(deposit_cmd, ctx.event_book())
        except CommandRejectedError as e:
            ctx.error = e

        # Then balance updated
        assert ctx.result.new_balance == 100
    """

    # Current aggregate being tested
    domain: str = ""
    root: bytes = b""

    # Event history (list of ProtoAny)
    events: list = field(default_factory=list)

    # Last command result
    result: Any = None
    error: CommandRejectedError | None = None

    # State after rebuild
    state: Any = None

    def event_book(self) -> EventBook:
        """Build EventBook from accumulated events.

        Creates an EventBook with proper sequencing from the
        events added via add_event().

        Returns:
            EventBook with cover, pages, and next_sequence set
        """
        pages = []
        for i, event_any in enumerate(self.events):
            pages.append(
                EventPage(
                    sequence=i,
                    event=event_any,
                    created_at=now(),
                )
            )
        return EventBook(
            cover=Cover(
                domain=self.domain,
                root=UUID(value=self.root),
            ),
            pages=pages,
            next_sequence=len(pages),
        )

    def add_event(self, event_msg, type_url_prefix: str = "type.googleapis.com/"):
        """Add an event to history.

        Packs the event message and appends to the event list.

        Args:
            event_msg: The protobuf event message to add
            type_url_prefix: URL prefix for type identification
        """
        self.events.append(pack_event(event_msg, type_url_prefix))

    def clear_events(self):
        """Clear all events from history."""
        self.events.clear()

    def clear_result(self):
        """Clear the last result and error."""
        self.result = None
        self.error = None

    def reset(self):
        """Reset context to initial state."""
        self.domain = ""
        self.root = b""
        self.events.clear()
        self.result = None
        self.error = None
        self.state = None


__all__ = ["ScenarioContext"]
