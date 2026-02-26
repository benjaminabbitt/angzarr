"""Base Saga class for event-driven command production.

Sagas translate events from one domain into commands for another domain.
They are stateless - each event is processed independently.

Router Pattern: Saga follows the SINGLE-DOMAIN OO pattern.
- One input domain: use @domain("name") class decorator
- One output_domain: commands go to a single target domain
- Uses @handles decorator for handler registration (with prepare=True for prepare phase)

Two-phase protocol support:
    1. Prepare: Declare destination aggregates needed (via @handles(EventType, prepare=True))
    2. Execute: Produce commands given source + destination state (via @handles(EventType))

Example usage (simple saga without destinations):
    from angzarr_client.saga import Saga, handles, domain

    @domain("order")
    class OrderFulfillmentSaga(Saga):
        name = "saga-order-fulfillment"
        output_domain = "fulfillment"

        @handles(OrderCompleted)
        def handle_completed(self, event: OrderCompleted) -> CreateShipment:
            return CreateShipment(order_id=event.order_id)

Example usage (saga with destinations):
    from angzarr_client.saga import Saga, handles, domain

    @domain("table")
    class TableHandSaga(Saga):
        name = "saga-table-hand"
        output_domain = "hand"

        @handles(HandStarted, prepare=True)
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
from .proto.angzarr import types_pb2 as types
from .router import (
    _pack_any,
    domain,
    handles,
    prepares,
)

# Re-export decorators
__all__ = ["Saga", "domain", "handles", "prepares"]


class Saga(ABC):
    """Base class for stateless event-to-command sagas.

    Router Pattern: Follows the SINGLE-DOMAIN OO pattern.

    Saga-specific additions:
    - output_domain: target domain for emitted commands
    - Auto-packing: returned commands are automatically packed into CommandBooks

    Provides:
    - Two-phase protocol: @handles(E, prepare=True) for destinations, @handles(E) for execution
    - Event dispatch via @handles decorated methods
    - Command packing into CommandBook
    - Descriptor generation for topology discovery

    Subclasses must:
    - Use `@domain("name")` class decorator (input domain to listen to)
    - Set `name` class attribute (e.g., "saga-order-fulfillment")
    - Set `output_domain` class attribute (domain to send commands to)
    - Decorate event handlers with `@handles(EventType)`
    - Optionally decorate prepare handlers with `@handles(EventType, prepare=True)`

    Usage (simple):
        @domain("order")
        class OrderFulfillmentSaga(Saga):
            name = "saga-order-fulfillment"
            output_domain = "fulfillment"

            @handles(OrderCompleted)
            def handle_completed(self, event: OrderCompleted) -> CreateShipment:
                return CreateShipment(order_id=event.order_id)

    Usage (with destinations):
        @domain("table")
        class TableHandSaga(Saga):
            name = "saga-table-hand"
            output_domain = "hand"

            @handles(HandStarted, prepare=True)
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
    output_domain: str
    _domain: str = None  # Set by @domain decorator
    _dispatch_table: dict[str, tuple[str, type]] = {}
    _prepare_table: dict[str, tuple[str, type]] = {}

    @property
    def input_domain(self) -> str:
        """Get input domain (from @domain decorator)."""
        return self._domain

    def __init_subclass__(cls, domain: str = None, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Set domain from __init_subclass__ kwarg
        if domain is not None:
            cls._domain = domain

        # Validate required class attributes
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define 'name' class attribute")

        if getattr(cls, "_domain", None) is None:
            raise TypeError(
                f"{cls.__name__} must specify domain: class {cls.__name__}(Saga, domain='x')"
            )

        if not getattr(cls, "output_domain", None):
            raise TypeError(
                f"{cls.__name__} must define 'output_domain' class attribute"
            )

        cls._dispatch_table = cls._build_dispatch_table()
        cls._prepare_table = cls._build_prepare_table()

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
        destinations: list[types.EventBook] = None,
    ) -> list[types.CommandBook]:
        """Dispatch event to matching @handles method.

        Args:
            event_any: Packed event as google.protobuf.Any
            root: Source aggregate root (passed to command cover)
            correlation_id: Correlation ID for the workflow
            destinations: Optional list of destination EventBooks

        Returns:
            List of CommandBooks to send.
        """
        type_url = event_any.type_url

        for full_name, (method_name, event_type) in self._dispatch_table.items():
            if type_url == TYPE_URL_PREFIX + full_name:
                # Unpack event
                event = event_type()
                event_any.Unpack(event)

                # Check if handler accepts destinations parameter
                method = getattr(self, method_name)
                sig = inspect.signature(method)
                params = list(sig.parameters.keys())

                # Call handler with or without destinations
                if "destinations" in params:
                    result = method(event, destinations=destinations or [])
                else:
                    result = method(event)

                # Pack result into CommandBooks
                return self._pack_commands(result, root, correlation_id, destinations)

        # No handler found - return empty (saga may not care about all events)
        return []

    def _pack_commands(
        self,
        result,
        root: bytes = None,
        correlation_id: str = "",
        destinations: list[types.EventBook] = None,
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
                domain=self.output_domain,
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
        saga = cls()
        destinations: list[types.Cover] = []

        for page in source.pages:
            if page.HasField("event"):
                destinations.extend(saga.prepare(page.event))

        return destinations

    @classmethod
    def execute(
        cls,
        source: types.EventBook,
        destinations: list[types.EventBook] = None,
    ) -> list[types.CommandBook]:
        """Phase 2: Process EventBook and return commands.

        Creates a saga instance and dispatches each event.
        This is the entry point for gRPC integration.

        Args:
            source: EventBook containing events to process.
            destinations: Optional list of destination EventBooks from prepare phase.

        Returns:
            List of CommandBooks to send.
        """
        saga = cls()
        root = source.cover.root.value if source.HasField("cover") else None
        correlation_id = source.cover.correlation_id if source.HasField("cover") else ""

        commands = []
        for page in source.pages:
            if page.HasField("event"):
                commands.extend(
                    saga.dispatch(page.event, root, correlation_id, destinations)
                )

        return commands
