"""DRY dispatch via router types.

CommandRouter replaces manual if/elif chains in aggregate handlers.
EventRouter replaces manual if/elif chains in saga event handlers.
Both auto-derive descriptors from their .on() registrations.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable, Generic, TypeVar

from google.protobuf import any_pb2

from angzarr import types_pb2 as types

S = TypeVar("S")

# Component type constants for Descriptor.
COMPONENT_AGGREGATE = "aggregate"
COMPONENT_SAGA = "saga"

# Error message constants.
ERRMSG_UNKNOWN_COMMAND = "Unknown command type"
ERRMSG_NO_COMMAND_PAGES = "No command pages"


# ============================================================================
# Descriptor types — mirrors angzarr ComponentDescriptor
# ============================================================================


@dataclass
class SubscriptionDesc:
    """Describes what a component subscribes to.

    Mirrors angzarr.Subscription. Will be replaced by the real proto type
    once generated code includes ComponentDescriptor.
    """

    domain: str
    event_types: list[str] = field(default_factory=list)


@dataclass
class Descriptor:
    """Describes a component for topology discovery.

    Mirrors angzarr.ComponentDescriptor. Will be replaced by the real proto
    type once generated code includes it.
    """

    name: str
    component_type: str
    inputs: list[SubscriptionDesc] = field(default_factory=list)


# ============================================================================
# CommandRouter — aggregate dispatch
# ============================================================================


class CommandRouter(Generic[S]):
    """DRY command dispatcher for aggregates.

    Matches command type_url suffixes and dispatches to registered handlers.
    Auto-derives descriptors from registrations.

    Takes a ContextualCommand, rebuilds state, matches the command's type_url
    suffix, dispatches to the registered handler, and wraps the result in
    a BusinessResponse.

    The handler signature:
        handler(cb: CommandBook, command_any: Any, state: S, seq: int) -> EventBook

    Example::

        router = (CommandRouter("cart", rebuild_state)
            .on("CreateCart", handle_create_cart)
            .on("AddItem", handle_add_item))

        # In Handle():
        response = router.dispatch(request)

        # For topology:
        desc = router.descriptor()
    """

    def __init__(self, domain: str, rebuild: Callable[[types.EventBook | None], S]) -> None:
        self.domain = domain
        self._rebuild = rebuild
        self._handlers: list[tuple[str, Callable]] = []

    def on(self, suffix: str, handler: Callable) -> CommandRouter[S]:
        """Register a handler for a command type_url suffix."""
        self._handlers.append((suffix, handler))
        return self

    def dispatch(self, cmd: types.ContextualCommand) -> types.BusinessResponse:
        """Dispatch a ContextualCommand to the matching handler.

        Extracts command + prior events, rebuilds state, matches type_url
        suffix, and calls the registered handler.

        Returns:
            BusinessResponse wrapping the handler's EventBook.

        Raises:
            ValueError: If no command pages or no handler matches.
        """
        command_book = cmd.command
        prior_events = cmd.events if cmd.HasField("events") else None

        state = self._rebuild(prior_events)
        seq = next_sequence(prior_events)

        if not command_book.pages:
            raise ValueError(ERRMSG_NO_COMMAND_PAGES)

        command_any = command_book.pages[0].command
        if not command_any.type_url:
            raise ValueError(ERRMSG_NO_COMMAND_PAGES)

        type_url = command_any.type_url
        for suffix, handler in self._handlers:
            if type_url.endswith(suffix):
                events = handler(command_book, command_any, state, seq)
                return types.BusinessResponse(events=events)

        raise ValueError(f"{ERRMSG_UNKNOWN_COMMAND}: {type_url}")

    def descriptor(self) -> Descriptor:
        """Build a component descriptor from registered handlers."""
        return Descriptor(
            name=self.domain,
            component_type=COMPONENT_AGGREGATE,
            inputs=[SubscriptionDesc(domain=self.domain, event_types=self.types())],
        )

    def types(self) -> list[str]:
        """Return registered command type suffixes."""
        return [suffix for suffix, _ in self._handlers]


# ============================================================================
# Helpers
# ============================================================================


def next_sequence(events: types.EventBook | None) -> int:
    """Compute the next event sequence number from prior events."""
    if events is None or not events.pages:
        return 0
    return len(events.pages)


# ============================================================================
# EventRouter — saga dispatch
# ============================================================================


class EventRouter:
    """DRY event dispatcher for sagas.

    Matches event type_url suffixes and dispatches to registered handlers.
    Auto-derives descriptors from registrations.

    Takes an EventBook, iterates its pages, matches type_url suffixes,
    and collects commands from handlers.

    The handler signature:
        handler(event: Any, root: UUID | None, correlation_id: str) -> list[CommandBook]

    Example::

        router = (EventRouter("fulfillment", "order")
            .output("fulfillment")
            .on("OrderCompleted", handle_order_completed))

        # In saga Execute:
        commands = router.dispatch(source_event_book)

        # For topology:
        desc = router.descriptor()
    """

    def __init__(self, name: str, input_domain: str) -> None:
        self.name = name
        self.input_domain = input_domain
        self.output_domains: list[str] = []
        self._handlers: list[tuple[str, Callable]] = []

    def output(self, domain: str) -> EventRouter:
        """Declare an output domain for this saga."""
        self.output_domains.append(domain)
        return self

    def on(self, suffix: str, handler: Callable) -> EventRouter:
        """Register a handler for an event type_url suffix."""
        self._handlers.append((suffix, handler))
        return self

    def dispatch(self, book: types.EventBook) -> list[types.CommandBook]:
        """Dispatch all events in an EventBook to registered handlers.

        Iterates pages, matches type_url suffixes, and collects commands.
        """
        root = book.cover.root if book.HasField("cover") else None
        correlation_id = book.cover.correlation_id if book.HasField("cover") else ""

        commands: list[types.CommandBook] = []
        for page in book.pages:
            if not page.HasField("event"):
                continue
            for suffix, handler in self._handlers:
                if page.event.type_url.endswith(suffix):
                    commands.extend(handler(page.event, root, correlation_id))
                    break
        return commands

    def descriptor(self) -> Descriptor:
        """Build a component descriptor from registered handlers."""
        return Descriptor(
            name=self.name,
            component_type=COMPONENT_SAGA,
            inputs=[SubscriptionDesc(domain=self.input_domain, event_types=self.types())],
        )

    def types(self) -> list[str]:
        """Return registered event type suffixes."""
        return [suffix for suffix, _ in self._handlers]
