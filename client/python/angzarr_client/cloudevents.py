"""CloudEvents support for Angzarr projectors.

CloudEvents projectors transform internal domain events into CloudEvents 1.0 format
for external consumption via HTTP webhooks or Kafka.

OO Pattern (CloudEventsProjector):
    from angzarr_client import CloudEvent, CloudEventsProjector

    @domain("player")
    class PlayerCloudEventsProjector(CloudEventsProjector):
        name = "prj-player-cloudevents"

        def on_player_registered(self, event: PlayerRegistered) -> CloudEvent | None:
            public = PublicPlayerRegistered(display_name=event.display_name)
            data = Any()
            data.Pack(public)
            return CloudEvent(type="com.poker.player.registered", data=data)

Functional Pattern (CloudEventsRouter):
    from angzarr_client import CloudEvent, CloudEventsRouter

    def handle_player_registered(event: PlayerRegistered) -> CloudEvent | None:
        public = PublicPlayerRegistered(display_name=event.display_name)
        data = Any()
        data.Pack(public)
        return CloudEvent(type="com.poker.player.registered", data=data)

    router = (
        CloudEventsRouter("prj-player-cloudevents", "player")
        .on("PlayerRegistered", handle_player_registered)
    )
"""

from __future__ import annotations

import inspect
from abc import ABC
from typing import Callable, Optional

from google.protobuf.any_pb2 import Any
from google.protobuf.message import Message

from .helpers import TYPE_URL_PREFIX
from .proto.angzarr import types_pb2 as types
from .proto.angzarr.cloudevents_pb2 import CloudEvent, CloudEventsResponse
from .router import domain

__all__ = [
    "CloudEvent",
    "CloudEventsProjector",
    "CloudEventsRouter",
    "CloudEventsResponse",
]


class CloudEventsProjector(ABC):
    """Base class for CloudEvents projectors using method naming convention.

    Handler methods are named `on_{event_type}` where event_type is the snake_case
    version of the protobuf message name (e.g., on_player_registered for PlayerRegistered).

    Subclasses must:
    - Set `name` class attribute
    - Use @domain decorator to set input domain
    - Define `on_{event_type}` methods that return CloudEvent or None

    Example:
        @domain("player")
        class PlayerCloudEventsProjector(CloudEventsProjector):
            name = "prj-player-cloudevents"

            def on_player_registered(self, event: PlayerRegistered) -> CloudEvent | None:
                public = PublicPlayerRegistered(display_name=event.display_name)
                data = Any()
                data.Pack(public)
                return CloudEvent(type="com.poker.player.registered", data=data)
    """

    name: str
    _domain: str = None  # Set by @domain decorator

    @property
    def input_domain(self) -> str:
        """Get input domain (from @domain decorator or __init__ argument)."""
        return self._domain

    def __init__(self, name: str = None, input_domain: str = None) -> None:
        """Initialize projector.

        Args:
            name: Projector name (optional, uses class attribute if not provided)
            input_domain: Input domain (optional, uses @domain decorator if not provided)
        """
        if name:
            self.name = name
        if input_domain:
            self._domain = input_domain

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

    def project(self, source: types.EventBook) -> list[CloudEvent]:
        """Process an EventBook and return CloudEvents.

        Args:
            source: EventBook containing events to process.

        Returns:
            List of CloudEvents for external publishing.
        """
        events = []

        for page in source.pages:
            if not page.HasField("event"):
                continue

            # Extract event type suffix from type_url
            type_url = page.event.type_url
            suffix = type_url.rsplit("/", 1)[-1].rsplit(".", 1)[-1]

            # Convert PascalCase to snake_case for method name
            method_name = f"on_{_pascal_to_snake(suffix)}"

            # Try to call handler method
            handler = getattr(self, method_name, None)
            if handler and callable(handler):
                # Need to unpack the event - try to infer type from type hints
                hints = getattr(handler, "__annotations__", {})
                event_types = [
                    v for k, v in hints.items() if k != "return" and isinstance(v, type)
                ]
                if event_types:
                    event_type = event_types[0]
                    event = event_type()
                    page.event.Unpack(event)
                    result = handler(event)
                    if result is not None:
                        events.append(result)

        return events


class CloudEventsRouter:
    """Functional router for CloudEvents projectors.

    Example:
        def handle_player_registered(event: PlayerRegistered) -> CloudEvent | None:
            public = PublicPlayerRegistered(display_name=event.display_name)
            data = Any()
            data.Pack(public)
            return CloudEvent(type="com.poker.player.registered", data=data)

        router = (
            CloudEventsRouter("prj-player-cloudevents", "player")
            .on("PlayerRegistered", handle_player_registered)
        )
    """

    def __init__(self, name: str, input_domain: str) -> None:
        self.name = name
        self.input_domain = input_domain
        self._handlers: dict[str, tuple[type, Callable]] = {}

    def on(
        self,
        event_suffix: str,
        handler: Callable[[Message], Optional[CloudEvent]],
    ) -> "CloudEventsRouter":
        """Register a handler for an event type.

        Args:
            event_suffix: Event type suffix to match (e.g., "PlayerRegistered")
            handler: Function that takes the event and returns CloudEvent or None

        Returns:
            Self for chaining.
        """
        # Get event type from type hints
        hints = getattr(handler, "__annotations__", {})
        event_types = [
            v for k, v in hints.items() if k != "return" and isinstance(v, type)
        ]
        event_type = event_types[0] if event_types else None
        self._handlers[event_suffix] = (event_type, handler)
        return self

    def project(self, source: types.EventBook) -> list[CloudEvent]:
        """Process an EventBook and return CloudEvents.

        Args:
            source: EventBook containing events to process.

        Returns:
            List of CloudEvents for external publishing.
        """
        events = []

        for page in source.pages:
            if not page.HasField("event"):
                continue

            # Extract event type suffix from type_url
            type_url = page.event.type_url
            suffix = type_url.rsplit("/", 1)[-1].rsplit(".", 1)[-1]

            handler_info = self._handlers.get(suffix)
            if handler_info:
                event_type, handler = handler_info
                if event_type:
                    event = event_type()
                    page.event.Unpack(event)
                    result = handler(event)
                else:
                    # No type hint, pass the Any directly
                    result = handler(page.event)

                if result is not None:
                    events.append(result)

        return events


def _pascal_to_snake(name: str) -> str:
    """Convert PascalCase to snake_case."""
    result = []
    for i, char in enumerate(name):
        if char.isupper() and i > 0:
            result.append("_")
        result.append(char.lower())
    return "".join(result)
