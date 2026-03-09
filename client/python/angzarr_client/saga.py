"""Base Saga class for event-driven command production.

Sagas translate events from one domain into commands for another domain.
They are stateless - each event is processed independently.

Router Pattern: Saga follows the SINGLE-DOMAIN OO pattern.
- One input domain: use @domain class decorator
- One output_domain: use @output_domain class decorator
- Uses @handles decorator for handler registration
- Uses @prepares decorator for prepare phase handlers

Two-phase protocol support:
    1. Prepare: Declare destination aggregates needed (via @prepares(EventType))
    2. Execute: Produce commands given source + destination state (via @handles(EventType))

Example usage (simple saga without destinations):
    from angzarr_client.saga import Saga, domain, output_domain, handles

    @domain("order")
    @output_domain("fulfillment")
    class OrderFulfillmentSaga(Saga):
        name = "saga-order-fulfillment"

        @handles(OrderCompleted)
        def handle_completed(self, event: OrderCompleted) -> CreateShipment:
            return CreateShipment(order_id=event.order_id)

Example usage (saga with destinations):
    from angzarr_client.saga import Saga, domain, output_domain, handles, prepares

    @domain("table")
    @output_domain("hand")
    class TableHandSaga(Saga):
        name = "saga-table-hand"

        @prepares(HandStarted)
        def prepare_hand(self, event: HandStarted) -> list[Cover]:
            return [Cover(domain="hand", root=UUID(value=event.hand_root))]

        @handles(HandStarted)
        def handle_hand_started(
            self, event: HandStarted, destinations: list[EventBook]
        ) -> DealCards:
            dest_seq = next_sequence(destinations[0]) if destinations else 0
            return DealCards(table_root=event.hand_root, ...)

"""

from __future__ import annotations

import inspect
from abc import ABC

from google.protobuf.any_pb2 import Any

from .helpers import TYPE_URL_PREFIX
from .proto.angzarr import saga_pb2
from .proto.angzarr import types_pb2 as types
from .router import (
    _pack_any,
    domain,
    handles,
    output_domain,
    prepares,
)

# Re-export decorators
__all__ = ["Saga", "domain", "output_domain", "handles", "prepares"]


class Saga(ABC):
    """Base class for stateless event-to-command sagas.

    Router Pattern: Follows the SINGLE-DOMAIN OO pattern.

    Saga-specific additions:
    - @output_domain: target domain for emitted commands
    - Auto-packing: returned commands are automatically packed into CommandBooks

    Provides:
    - Two-phase protocol: @prepares(E) for destinations, @handles(E) for execution
    - Event dispatch via @handles decorated methods
    - Command packing into CommandBook
    - Descriptor generation for topology discovery

    Subclasses must:
    - Use @domain class decorator for input domain
    - Use @output_domain class decorator for output domain
    - Set `name` class attribute (e.g., "saga-order-fulfillment")
    - Decorate event handlers with `@handles(EventType)`
    - Optionally decorate prepare handlers with `@prepares(EventType)`

    Usage (simple):
        @domain("order")
        @output_domain("fulfillment")
        class OrderFulfillmentSaga(Saga):
            name = "saga-order-fulfillment"

            @handles(OrderCompleted)
            def handle_completed(self, event: OrderCompleted) -> CreateShipment:
                return CreateShipment(order_id=event.order_id)

    Usage (with destinations):
        @domain("table")
        @output_domain("hand")
        class TableHandSaga(Saga):
            name = "saga-table-hand"

            @prepares(HandStarted)
            def prepare_hand(self, event: HandStarted) -> list[Cover]:
                return [Cover(domain="hand", root=UUID(value=event.hand_root))]

            @handles(HandStarted)
            def handle_hand_started(
                self, event: HandStarted, destinations: list[EventBook]
            ) -> DealCards:
                dest_seq = next_sequence(destinations[0]) if destinations else 0
                return DealCards(...)

    """

    name: str
    _domain: str = None  # Set by @domain decorator
    _output_domain: str = None  # Set by @output_domain decorator
    _dispatch_table: dict[str, tuple[str, type]] = {}
    _prepare_table: dict[str, tuple[str, type]] = {}
    _validated: bool = False

    def __init__(self) -> None:
        """Initialize saga instance with empty event accumulator."""
        self._events: list[types.EventBook] = []

    def emit_event(self, event: types.EventBook) -> None:
        """Emit a fact (event to inject to another aggregate).

        Args:
            event: EventBook containing the event to inject.
                   The Cover should specify target domain and root.
        """
        self._events.append(event)

    @property
    def input_domain(self) -> str:
        """Get input domain (from @domain decorator)."""
        return self._domain

    @classmethod
    def _get_output_domain(cls) -> str:
        """Get output domain (from @output_domain decorator)."""
        return cls._output_domain

    @classmethod
    def _ensure_configured(cls) -> None:
        """Validate configuration at first use (lazy validation)."""
        if cls._validated:
            return

        if getattr(cls, "_domain", None) is None:
            raise TypeError(f"{cls.__name__} must use @domain decorator")

        if getattr(cls, "_output_domain", None) is None:
            raise TypeError(f"{cls.__name__} must use @output_domain decorator")

        cls._validated = True

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Validate name attribute (required at definition time)
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define 'name' class attribute")

        # Build dispatch tables (decorators have run by now)
        cls._dispatch_table = cls._build_dispatch_table()
        cls._prepare_table = cls._build_prepare_table()
        cls._validated = False

    @classmethod
    def _build_dispatch_table(cls) -> dict[str, tuple[str, type]]:
        """Scan for @handles methods and build dispatch table."""
        table = {}
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if callable(attr) and getattr(attr, "_is_handler", False):
                event_type = attr._event_type
                full_name = event_type.DESCRIPTOR.full_name
                if full_name in table:
                    raise TypeError(
                        f"{cls.__name__}: duplicate handler for {full_name}"
                    )
                table[full_name] = (attr_name, event_type)
        return table

    @classmethod
    def _build_prepare_table(cls) -> dict[str, tuple[str, type]]:
        """Scan for @prepares methods and build prepare table."""
        table = {}
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if callable(attr) and getattr(attr, "_is_prepare_handler", False):
                event_type = attr._event_type
                full_name = event_type.DESCRIPTOR.full_name
                if full_name in table:
                    raise TypeError(
                        f"{cls.__name__}: duplicate prepare handler for {full_name}"
                    )
                table[full_name] = (attr_name, event_type)
        return table

    def prepare(self, event_any: Any) -> list[types.Cover]:
        """Prepare destinations for an event.

        Dispatches to @prepares decorated method if one exists.

        Args:
            event_any: Packed event as google.protobuf.Any

        Returns:
            List of Covers identifying destination aggregates.
        """
        type_url = event_any.type_url

        for full_name, (method_name, event_type) in self._prepare_table.items():
            if type_url == TYPE_URL_PREFIX + full_name:
                # Unpack event
                event = event_type()
                event_any.Unpack(event)

                # Call prepare handler
                result = getattr(self, method_name)(event)
                return result if result else []

        return []

    def dispatch(
        self,
        event_any: Any,
        root: bytes = None,
        correlation_id: str = "",
    ) -> list[types.CommandBook]:
        """Dispatch event to matching @handles method.

        Args:
            event_any: Packed event as google.protobuf.Any
            root: Source aggregate root (passed to command cover)
            correlation_id: Correlation ID for the workflow

        Returns:
            List of CommandBooks to send.
        """
        type_url = event_any.type_url

        for full_name, (method_name, event_type) in self._dispatch_table.items():
            if type_url == TYPE_URL_PREFIX + full_name:
                # Unpack event
                event = event_type()
                event_any.Unpack(event)

                # Call handler
                result = getattr(self, method_name)(event)

                # Pack result into CommandBooks
                return self._pack_commands(result, root, correlation_id)

        # No handler found - return empty (saga may not care about all events)
        return []

    def _pack_commands(
        self,
        result,
        root: bytes = None,
        correlation_id: str = "",
    ) -> list[types.CommandBook]:
        """Pack command(s) into CommandBooks."""
        if result is None:
            return []

        # Handle pre-packed CommandBooks (advanced usage)
        if isinstance(result, types.CommandBook):
            return [result]
        if (
            isinstance(result, list)
            and result
            and isinstance(result[0], types.CommandBook)
        ):
            return result

        commands = result if isinstance(result, tuple) else (result,)
        books = []

        for cmd in commands:
            cmd_any = _pack_any(cmd)
            cover = types.Cover(
                domain=self._get_output_domain(),
                correlation_id=correlation_id,
            )
            if root:
                cover.root.value = root

            book = types.CommandBook(
                cover=cover,
                pages=[types.CommandPage(command=cmd_any)],
            )
            books.append(book)

        return books

    @classmethod
    def prepare_destinations(cls, source: types.EventBook) -> list[types.Cover]:
        """Phase 1: Declare destination aggregates needed.

        Args:
            source: EventBook containing events to process.

        Returns:
            List of Covers identifying destination aggregates to fetch.
        """
        cls._ensure_configured()
        saga = cls()
        destinations: list[types.Cover] = []

        for page in source.pages:
            if page.HasField("event"):
                destinations.extend(saga.prepare(page.event))

        return destinations

    @classmethod
    def handle(
        cls,
        source: types.EventBook,
    ) -> saga_pb2.SagaResponse:
        """Handle source events and produce commands.

        Creates a saga instance and dispatches each event.
        This is the entry point for gRPC integration.

        Args:
            source: EventBook containing events to process.

        Returns:
            SagaResponse containing commands and events.
        """
        cls._ensure_configured()
        saga = cls()
        root = source.cover.root.value if source.HasField("cover") else None
        correlation_id = source.cover.correlation_id if source.HasField("cover") else ""

        commands = []
        for page in source.pages:
            event_any = page.event if page.HasField("event") else None
            if (
                event_any is None
                and hasattr(page, "payload")
                and page.HasField("payload")
            ):
                # New proto: event is in payload oneof
                if hasattr(page.payload, "event"):
                    event_any = page.payload.event
                elif hasattr(page, "GetEvent"):
                    event_any = page.GetEvent()
            if event_any:
                commands.extend(saga.dispatch(event_any, root, correlation_id))

        return saga_pb2.SagaResponse(commands=commands, events=saga._events)

    @classmethod
    def execute(
        cls,
        source: types.EventBook,
        destinations: list[types.EventBook] = None,
    ) -> saga_pb2.SagaResponse:
        """Deprecated: Use handle() instead.

        Kept for backwards compatibility.
        """
        return cls.handle(source)
