"""Base Projector class for event-driven read model updates.

Projectors consume events and produce projections (read model updates).
They are stateless event consumers that build query-optimized views.

Router Pattern: Projector follows the OO pattern.
- Single domain: use @domain class decorator
- Multi-domain: use input_domains class attribute (list of domains)
- Uses @handles decorator for event handler registration
- Stateless: each event projected independently
- External output: writes to read models, files, external systems

Example usage (single domain):
    from angzarr_client.projector import Projector, domain, handles

    @domain("inventory")
    class InventoryStockProjector(Projector):
        name = "projector-inventory-stock"

        @handles(StockUpdated)
        def project_stock(self, event: StockUpdated) -> Projection:
            # Pack projection data into Any
            data = StockLevel(sku=event.sku, quantity=event.quantity)
            data_any = Any()
            data_any.Pack(data)
            return Projection(projector=self.name, projection=data_any)

Example usage (multi-domain):
    from angzarr_client.projector import Projector, handles

    class OutputProjector(Projector):
        name = "output"
        input_domains = ["player", "table", "hand"]

        @handles(PlayerRegistered)
        def project_registered(self, event: PlayerRegistered) -> Projection:
            write_log(f"PLAYER registered: {event.display_name}")
            return Projection(projector=self.name)

"""

from __future__ import annotations

import inspect
from abc import ABC

from google.protobuf.any_pb2 import Any

from .helpers import TYPE_URL_PREFIX
from .proto.angzarr import types_pb2 as types
from .router import domain, handles

# Re-export decorators
__all__ = ["Projector", "domain", "handles"]


class Projector(ABC):
    """Base class for event-driven projectors.

    Router Pattern: Follows the OO pattern.

    Projector-specific notes:
    - Single domain: use @domain class decorator
    - Multi-domain: use input_domains class attribute
    - Stateless: each event projected independently
    - External output: writes to read models, external systems

    Provides:
    - Event dispatch via @handles decorated methods
    - Projection building
    - Descriptor generation for topology discovery

    Subclasses must:
    - Set `name` class attribute (e.g., "projector-inventory-stock")
    - Use @domain decorator (single domain) OR set `input_domains` (list)
    - Decorate event handlers with `@handles(EventType)`

    Usage (single domain):
        @domain("inventory")
        class InventoryStockProjector(Projector):
            name = "projector-inventory-stock"

            @handles(StockUpdated)
            def project_stock(self, event: StockUpdated) -> Projection:
                # Pack projection data into Any
                data = StockLevel(sku=event.sku, quantity=event.quantity)
                data_any = Any()
                data_any.Pack(data)
                return Projection(projector=self.name, projection=data_any)

    Usage (multi-domain):
        class OutputProjector(Projector):
            name = "output"
            input_domains = ["player", "table", "hand"]

            @handles(PlayerRegistered)
            def project_registered(self, event: PlayerRegistered) -> Projection:
                ...

    """

    name: str
    _domain: str = None  # Set by @domain decorator
    input_domains: list[str] = None
    _dispatch_table: dict[str, tuple[str, type]] = {}
    _validated: bool = False

    @property
    def input_domain(self) -> str:
        """Get input domain (from @domain decorator)."""
        return self._domain

    @classmethod
    def _ensure_configured(cls) -> None:
        """Validate configuration at first use (lazy validation)."""
        if cls._validated:
            return

        has_domain = getattr(cls, "_domain", None) is not None
        has_multi = getattr(cls, "input_domains", None) is not None

        if not has_domain and not has_multi:
            raise TypeError(
                f"{cls.__name__} must use @domain decorator or set input_domains"
            )

        cls._validated = True

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Validate name attribute (required at definition time)
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define 'name' class attribute")

        # Build dispatch table (decorators have run by now)
        cls._dispatch_table = cls._build_dispatch_table()
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

    def dispatch(self, event_any: Any) -> types.Projection:
        """Dispatch event to matching @handles method.

        Args:
            event_any: Packed event as google.protobuf.Any

        Returns:
            Projection result, or empty Projection if no handler matches.
        """
        type_url = event_any.type_url

        for full_name, (method_name, event_type) in self._dispatch_table.items():
            if type_url == TYPE_URL_PREFIX + full_name:
                # Unpack event
                event = event_type()
                event_any.Unpack(event)

                # Call handler
                result = getattr(self, method_name)(event)
                return result if result else types.Projection()

        # No handler found
        return types.Projection()

    @classmethod
    def handle(cls, source: types.EventBook) -> types.Projection:
        """Process an EventBook and return projection from last handled event.

        Creates a projector instance and dispatches each event.
        Returns the projection from the last successfully handled event.

        Args:
            source: EventBook containing events to process.

        Returns:
            Projection from the last handled event, or empty Projection.
        """
        cls._ensure_configured()
        projector = cls()
        last_projection = types.Projection()

        for page in source.pages:
            if page.HasField("event"):
                result = projector.dispatch(page.event)
                if result.projector:  # Non-empty projection
                    last_projection = result

        return last_projection
