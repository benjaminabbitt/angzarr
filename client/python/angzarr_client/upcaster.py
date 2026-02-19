"""Base Upcaster class for event version transformation.

Upcasters transform old event versions to current versions during replay.
They enable schema evolution without breaking existing event stores.

Example usage:
    from angzarr_client import Upcaster, upcasts

    class OrderUpcaster(Upcaster):
        name = "upcaster-order"
        domain = "order"

        @upcasts(OrderCreatedV1, OrderCreated)
        def upcast_created(self, old: OrderCreatedV1) -> OrderCreated:
            return OrderCreated(
                order_id=old.order_id,
                customer_id=old.customer_id,
                total=0,  # New field with default
            )
"""

from __future__ import annotations

import inspect
from abc import ABC
from typing import Callable

from google.protobuf import any_pb2

from .proto.angzarr import types_pb2 as types

__all__ = ["Upcaster", "upcasts"]


def upcasts(from_type: type, to_type: type):
    """Decorator for upcaster transformation methods.

    Registers the method as a handler that transforms from_type to to_type.
    Validates that type hints match the decorator arguments.

    Args:
        from_type: The old event version to transform from.
        to_type: The new event version to transform to.

    Example:
        @upcasts(OrderCreatedV1, OrderCreated)
        def upcast_created(self, old: OrderCreatedV1) -> OrderCreated:
            return OrderCreated(...)

    Raises:
        TypeError: If type hints don't match decorator arguments.
    """

    def decorator(method: Callable) -> Callable:
        # Validate type hints
        import typing

        hints = typing.get_type_hints(method)
        sig = inspect.signature(method)
        params = list(sig.parameters.keys())

        if len(params) < 2:  # self + old event
            raise TypeError(f"{method.__name__}: must have old event parameter")

        old_param = params[1]
        if old_param not in hints:
            raise TypeError(f"{method.__name__}: missing type hint for '{old_param}'")

        if hints[old_param] != from_type:
            raise TypeError(
                f"{method.__name__}: @upcasts({from_type.__name__}, ...) "
                f"doesn't match type hint {hints[old_param].__name__}"
            )

        # Check return type if present
        if "return" in hints and hints["return"] != to_type:
            raise TypeError(
                f"{method.__name__}: @upcasts(..., {to_type.__name__}) "
                f"doesn't match return hint {hints['return'].__name__}"
            )

        method._is_upcaster = True
        method._from_type = from_type
        method._to_type = to_type
        return method

    return decorator


class Upcaster(ABC):
    """Base class for event version transformation.

    Provides:
    - Event dispatch via @upcasts decorated methods
    - Version transformation
    - Descriptor generation for topology discovery

    Subclasses must:
    - Set `name` class attribute (e.g., "upcaster-order")
    - Set `domain` class attribute (domain to handle)
    - Decorate transformation methods with `@upcasts(OldType, NewType)`

    Usage:
        class OrderUpcaster(Upcaster):
            name = "upcaster-order"
            domain = "order"

            @upcasts(OrderCreatedV1, OrderCreated)
            def upcast_created(self, old: OrderCreatedV1) -> OrderCreated:
                return OrderCreated(
                    order_id=old.order_id,
                    customer_id=old.customer_id,
                    total=0,
                )
    """

    name: str
    domain: str
    _dispatch_table: dict[str, tuple[str, type, type]] = {}  # suffix -> (method, from_type, to_type)

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Validate required class attributes
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define 'name' class attribute")
        if not getattr(cls, "domain", None):
            raise TypeError(f"{cls.__name__} must define 'domain' class attribute")

        cls._dispatch_table = cls._build_dispatch_table()

    @classmethod
    def _build_dispatch_table(cls) -> dict[str, tuple[str, type, type]]:
        """Scan for @upcasts methods and build dispatch table."""
        table = {}
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if callable(attr) and getattr(attr, "_is_upcaster", False):
                from_type = attr._from_type
                to_type = attr._to_type
                suffix = from_type.__name__
                if suffix in table:
                    raise TypeError(f"{cls.__name__}: duplicate upcaster for {suffix}")
                table[suffix] = (attr_name, from_type, to_type)
        return table

    def upcast(self, event_any: any_pb2.Any) -> any_pb2.Any:
        """Transform a single event to current version.

        Args:
            event_any: Packed event as google.protobuf.Any

        Returns:
            Transformed event (new Any), or original if no transformation needed.
        """
        type_url = event_any.type_url

        for suffix, (method_name, from_type, to_type) in self._dispatch_table.items():
            if type_url.endswith(suffix):
                # Unpack old event
                old_event = from_type()
                event_any.Unpack(old_event)

                # Transform
                new_event = getattr(self, method_name)(old_event)

                # Pack new event
                new_any = any_pb2.Any()
                new_any.Pack(new_event)
                return new_any

        # No transformation needed
        return event_any

    @classmethod
    def handle(cls, events: list[types.EventPage]) -> list[types.EventPage]:
        """Transform a list of events to current versions.

        Args:
            events: List of EventPages to transform.

        Returns:
            List of EventPages with transformed events.
        """
        upcaster = cls()
        result = []

        for page in events:
            if page.HasField("event"):
                new_event = upcaster.upcast(page.event)
                new_page = types.EventPage(event=new_event, sequence=page.sequence)
                new_page.created_at.CopyFrom(page.created_at)
                result.append(new_page)
            else:
                result.append(page)

        return result

