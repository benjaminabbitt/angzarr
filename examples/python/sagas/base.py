"""Base saga infrastructure."""

from abc import ABC, abstractmethod
from dataclasses import dataclass

from angzarr_client.proto.angzarr import types_pb2 as types


from typing import Optional


@dataclass
class SagaContext:
    """Context passed to saga handlers."""

    event_book: types.EventBook
    event_type: str
    aggregate_type: str
    aggregate_root: bytes
    # Destination event books keyed by (domain, root_hex)
    destination_events: Optional[dict] = None

    def next_sequence_for(self, domain: str, root: bytes) -> int:
        """Get next sequence for a destination aggregate."""
        if self.destination_events is None:
            return 0  # Fallback for tests
        key = (domain, root.hex())
        dest_book = self.destination_events.get(key)
        return dest_book.next_sequence if dest_book else 0


class Saga(ABC):
    """
    Base class for sagas.

    Sagas are event handlers that coordinate across aggregates/domains.
    They subscribe to events and can:
    1. Emit commands to other aggregates
    2. Update read models (projectors)
    3. Trigger external integrations
    """

    @property
    @abstractmethod
    def name(self) -> str:
        """Return the saga name for logging."""
        pass

    @property
    @abstractmethod
    def subscribed_events(self) -> list[str]:
        """Return list of event type names this saga handles."""
        pass

    @abstractmethod
    def handle(self, context: SagaContext) -> list[types.CommandBook]:
        """
        Handle an event and optionally return commands to execute.

        Returns:
            List of CommandBooks to execute in other aggregates.
            Empty list if no commands need to be sent.
        """
        pass


class SagaRouter:
    """
    Routes events to registered sagas.

    This is the central coordinator that:
    1. Receives events from aggregates
    2. Dispatches to matching sagas
    3. Collects resulting commands for execution
    """

    def __init__(self):
        self._sagas: list[Saga] = []
        self._event_handlers: dict[str, list[Saga]] = {}

    def register(self, saga: Saga) -> "SagaRouter":
        """Register a saga."""
        self._sagas.append(saga)
        for event_type in saga.subscribed_events:
            if event_type not in self._event_handlers:
                self._event_handlers[event_type] = []
            self._event_handlers[event_type].append(saga)
        return self

    def route(
        self, event_book: types.EventBook, aggregate_type: str
    ) -> list[types.CommandBook]:
        """
        Route events to matching sagas.

        Args:
            event_book: The event book containing events
            aggregate_type: Type of aggregate that emitted the events

        Returns:
            List of CommandBooks from all sagas that handled these events
        """
        commands = []

        # Collect event types present in this book
        event_types_in_book = set()
        for page in event_book.pages:
            event_type = self._extract_event_type(page.event.type_url)
            event_types_in_book.add(event_type)

        # Call each saga once for each event type it handles
        sagas_called = set()
        for event_type in event_types_in_book:
            if event_type not in self._event_handlers:
                continue

            for saga in self._event_handlers[event_type]:
                # Only call each saga once per event_book
                if saga.name in sagas_called:
                    continue
                sagas_called.add(saga.name)

                context = SagaContext(
                    event_book=event_book,
                    event_type=event_type,
                    aggregate_type=aggregate_type,
                    aggregate_root=event_book.cover.root.value if event_book.cover.root else b"",
                )

                try:
                    saga_commands = saga.handle(context)
                    commands.extend(saga_commands)
                except Exception as e:
                    # Log but don't fail - sagas should be resilient
                    print(f"Saga {saga.name} failed: {e}")

        return commands

    def _extract_event_type(self, type_url: str) -> str:
        """Extract event type name from type_url."""
        # Format: "type.googleapis.com/examples.EventName"
        if "." in type_url:
            return type_url.split(".")[-1]
        return type_url
