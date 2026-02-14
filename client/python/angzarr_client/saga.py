"""Base Saga class for event-driven command production.

Sagas translate events from one domain into commands for another domain.
They are stateless - each event is processed independently.

Example usage:
    from angzarr_client import Saga, reacts_to

    class OrderFulfillmentSaga(Saga):
        name = "saga-order-fulfillment"
        input_domain = "order"
        output_domain = "fulfillment"

        @reacts_to(OrderCompleted)
        def handle_completed(self, event: OrderCompleted) -> CreateShipment:
            return CreateShipment(order_id=event.order_id)
"""

from __future__ import annotations

import inspect
from abc import ABC

from google.protobuf.any_pb2 import Any

from .proto.angzarr import types_pb2 as types
from .router import (
    COMPONENT_SAGA,
    Descriptor,
    TargetDesc,
    _pack_any,
    reacts_to,  # Re-export for convenience
)

# Re-export decorator
__all__ = ["Saga", "reacts_to"]


class Saga(ABC):
    """Base class for stateless event-to-command sagas.

    Provides:
    - Event dispatch via @reacts_to decorated methods
    - Command packing into CommandBook
    - Descriptor generation for topology discovery

    Subclasses must:
    - Set `name` class attribute (e.g., "saga-order-fulfillment")
    - Set `input_domain` class attribute (domain to listen to)
    - Set `output_domain` class attribute (domain to send commands to)
    - Decorate event handlers with `@reacts_to(EventType)`

    Usage:
        class OrderFulfillmentSaga(Saga):
            name = "saga-order-fulfillment"
            input_domain = "order"
            output_domain = "fulfillment"

            @reacts_to(OrderCompleted)
            def handle_completed(self, event: OrderCompleted) -> CreateShipment:
                return CreateShipment(order_id=event.order_id)
    """

    name: str
    input_domain: str
    output_domain: str
    _dispatch_table: dict[str, tuple[str, type]] = {}

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Validate required class attributes
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define 'name' class attribute")
        if not getattr(cls, "input_domain", None):
            raise TypeError(f"{cls.__name__} must define 'input_domain' class attribute")
        if not getattr(cls, "output_domain", None):
            raise TypeError(f"{cls.__name__} must define 'output_domain' class attribute")

        cls._dispatch_table = cls._build_dispatch_table()

    @classmethod
    def _build_dispatch_table(cls) -> dict[str, tuple[str, type]]:
        """Scan for @reacts_to methods and build dispatch table."""
        table = {}
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if callable(attr) and getattr(attr, "_is_handler", False):
                event_type = attr._event_type
                suffix = event_type.__name__
                if suffix in table:
                    raise TypeError(f"{cls.__name__}: duplicate handler for {suffix}")
                table[suffix] = (attr_name, event_type)
        return table

    def dispatch(self, event_any: Any, root: bytes = None, correlation_id: str = "") -> list[types.CommandBook]:
        """Dispatch event to matching @reacts_to method.

        Args:
            event_any: Packed event as google.protobuf.Any
            root: Source aggregate root (passed to command cover)
            correlation_id: Correlation ID for the workflow

        Returns:
            List of CommandBooks to send.

        Raises:
            ValueError: If no handler matches the event type.
        """
        type_url = event_any.type_url

        for suffix, (method_name, event_type) in self._dispatch_table.items():
            if type_url.endswith(suffix):
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
        self, result, root: bytes = None, correlation_id: str = ""
    ) -> list[types.CommandBook]:
        """Pack command(s) into CommandBooks."""
        if result is None:
            return []

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
    def execute(cls, source: types.EventBook) -> list[types.CommandBook]:
        """Process an EventBook and return commands.

        Creates a saga instance and dispatches each event.
        This is the entry point for gRPC integration.

        Args:
            source: EventBook containing events to process.

        Returns:
            List of CommandBooks to send.
        """
        saga = cls()
        root = source.cover.root.value if source.HasField("cover") else None
        correlation_id = source.cover.correlation_id if source.HasField("cover") else ""

        commands = []
        for page in source.pages:
            if page.HasField("event"):
                commands.extend(saga.dispatch(page.event, root, correlation_id))

        return commands

    @classmethod
    def descriptor(cls) -> Descriptor:
        """Build component descriptor for topology discovery."""
        return Descriptor(
            name=cls.name,
            component_type=COMPONENT_SAGA,
            inputs=[TargetDesc(domain=cls.input_domain, types=list(cls._dispatch_table.keys()))],
        )
