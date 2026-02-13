"""DRY dispatch via router types.

CommandRouter replaces manual if/elif chains in aggregate handlers.
EventRouter replaces manual if/elif chains in saga event handlers.
Both auto-derive descriptors from their .on() registrations.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any, Callable, Generic, TypeVar

from google.protobuf import any_pb2

from .proto.angzarr import aggregate_pb2 as aggregate
from .proto.angzarr import types_pb2 as types

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
class TargetDesc:
    """Describes what a component subscribes to (inputs) or sends to (outputs).

    Mirrors angzarr.Target proto message.
    """

    domain: str
    types: list[str] = field(default_factory=list)


@dataclass
class Descriptor:
    """Describes a component for topology discovery.

    Mirrors angzarr.ComponentDescriptor. Will be replaced by the real proto
    type once generated code includes it.
    """

    name: str
    component_type: str
    inputs: list[TargetDesc] = field(default_factory=list)


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

    def __init__(
        self, domain: str, rebuild: Callable[[types.EventBook | None], S]
    ) -> None:
        self.domain = domain
        self._rebuild = rebuild
        self._handlers: list[tuple[str, Callable]] = []

    def on(self, suffix: str, handler: Callable) -> CommandRouter[S]:
        """Register a handler for a command type_url suffix."""
        self._handlers.append((suffix, handler))
        return self

    def dispatch(self, cmd: types.ContextualCommand) -> aggregate.BusinessResponse:
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
                return aggregate.BusinessResponse(events=events)

        raise ValueError(f"{ERRMSG_UNKNOWN_COMMAND}: {type_url}")

    def descriptor(self) -> Descriptor:
        """Build a component descriptor from registered handlers."""
        return Descriptor(
            name=self.domain,
            component_type=COMPONENT_AGGREGATE,
            inputs=[TargetDesc(domain=self.domain, types=self.types())],
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

    Two-phase protocol support:
        1. prepare_destinations(source) -> list of Covers to fetch
        2. dispatch(source, destinations) -> list of CommandBooks

    The handler signature:
        handler(event: Any, root: UUID | None, correlation_id: str, destinations: list[EventBook]) -> list[CommandBook]

    The prepare handler signature:
        prepare_handler(event: Any, root: UUID | None) -> list[Cover]

    Example::

        router = (EventRouter("fulfillment", "order")
            .output("fulfillment")
            .prepare("OrderCompleted", prepare_order)
            .on("OrderCompleted", handle_order_completed))

        # In saga Prepare:
        covers = router.prepare_destinations(source_event_book)

        # In saga Execute:
        commands = router.dispatch(source_event_book, destinations)

        # For topology:
        desc = router.descriptor()
    """

    def __init__(self, name: str, input_domain: str) -> None:
        self.name = name
        self.input_domain = input_domain
        self._output_targets: dict[str, list[str]] = {}
        self._handlers: list[tuple[str, Callable]] = []
        self._prepare_handlers: dict[str, Callable] = {}

    def sends(self, domain: str, command_type: str) -> EventRouter:
        """Declare an output domain and command type this saga produces.

        Call multiple times for multiple command types or domains.
        """
        if domain not in self._output_targets:
            self._output_targets[domain] = []
        self._output_targets[domain].append(command_type)
        return self

    def prepare(self, suffix: str, handler: Callable) -> EventRouter:
        """Register a prepare handler for an event type_url suffix.

        The prepare handler returns a list of Covers identifying destinations
        that should be fetched before the main handler executes.
        """
        self._prepare_handlers[suffix] = handler
        return self

    def on(self, suffix: str, handler: Callable) -> EventRouter:
        """Register a handler for an event type_url suffix."""
        self._handlers.append((suffix, handler))
        return self

    def prepare_destinations(self, book: types.EventBook) -> list[types.Cover]:
        """Get destinations needed for the given source events.

        Iterates pages, matches type_url suffixes against prepare handlers,
        and collects destination Covers.
        """
        root = book.cover.root if book.HasField("cover") else None

        destinations: list[types.Cover] = []
        for page in book.pages:
            if not page.HasField("event"):
                continue
            for suffix, handler in self._prepare_handlers.items():
                if page.event.type_url.endswith(suffix):
                    destinations.extend(handler(page.event, root))
                    break
        return destinations

    def dispatch(
        self,
        book: types.EventBook,
        destinations: list[types.EventBook] | None = None,
    ) -> list[types.CommandBook]:
        """Dispatch all events in an EventBook to registered handlers.

        Iterates pages, matches type_url suffixes, and collects commands.

        Args:
            book: Source EventBook containing events to process
            destinations: Optional list of destination EventBooks for two-phase protocol
        """
        root = book.cover.root if book.HasField("cover") else None
        correlation_id = book.cover.correlation_id if book.HasField("cover") else ""
        dests = destinations or []

        commands: list[types.CommandBook] = []
        for page in book.pages:
            if not page.HasField("event"):
                continue
            for suffix, handler in self._handlers:
                if page.event.type_url.endswith(suffix):
                    commands.extend(handler(page.event, root, correlation_id, dests))
                    break
        return commands

    def descriptor(self) -> Descriptor:
        """Build a component descriptor from registered handlers."""
        return Descriptor(
            name=self.name,
            component_type=COMPONENT_SAGA,
            inputs=[
                TargetDesc(domain=self.input_domain, types=self.types())
            ],
        )

    def types(self) -> list[str]:
        """Return registered event type suffixes."""
        return [suffix for suffix, _ in self._handlers]

    def output_domains(self) -> list[str]:
        """Return the list of output domain names."""
        return list(self._output_targets.keys())

    def output_types(self, domain: str) -> list[str]:
        """Return the command types for a given output domain."""
        return self._output_targets.get(domain, [])
