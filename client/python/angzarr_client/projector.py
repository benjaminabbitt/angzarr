"""Base Projector class for event-driven read model updates.

Projectors consume events and produce projections (read model updates).
They are stateless event consumers that build query-optimized views.

Example usage (single domain):
    from angzarr_client import Projector, projects

    class InventoryStockProjector(Projector):
        name = "projector-inventory-stock"
        input_domain = "inventory"

        @projects(StockUpdated)
        def project_stock(self, event: StockUpdated) -> Projection:
            # Pack projection data into Any
            data = StockLevel(sku=event.sku, quantity=event.quantity)
            data_any = Any()
            data_any.Pack(data)
            return Projection(projector=self.name, projection=data_any)

Example usage (multi-domain):
    from angzarr_client import Projector, projects

    class OutputProjector(Projector):
        name = "output"
        input_domains = ["player", "table", "hand"]

        @projects(PlayerRegistered)
        def project_registered(self, event: PlayerRegistered) -> Projection:
            write_log(f"PLAYER registered: {event.display_name}")
            return Projection(projector=self.name)
"""

from __future__ import annotations

import inspect
from abc import ABC

from google.protobuf.any_pb2 import Any

from .proto.angzarr import types_pb2 as types
from .router import Descriptor, TargetDesc, projects

# Re-export decorator
__all__ = ["Projector", "projects"]


class Projector(ABC):
    """Base class for event-driven projectors.

    Provides:
    - Event dispatch via @projects decorated methods
    - Projection building
    - Descriptor generation for topology discovery

    Subclasses must:
    - Set `name` class attribute (e.g., "projector-inventory-stock")
    - Set `input_domain` (single domain) OR `input_domains` (list of domains)
    - Decorate event handlers with `@projects(EventType)`

    Usage (single domain):
        class InventoryStockProjector(Projector):
            name = "projector-inventory-stock"
            input_domain = "inventory"

            @projects(StockUpdated)
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

            @projects(PlayerRegistered)
            def project_registered(self, event: PlayerRegistered) -> Projection:
                ...
    """

    name: str
    input_domain: str = None
    input_domains: list[str] = None
    _dispatch_table: dict[str, tuple[str, type]] = {}

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Validate required class attributes
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define 'name' class attribute")

        # Require either input_domain or input_domains
        has_single = getattr(cls, "input_domain", None) is not None
        has_multi = getattr(cls, "input_domains", None) is not None
        if not has_single and not has_multi:
            raise TypeError(
                f"{cls.__name__} must define 'input_domain' or 'input_domains' class attribute"
            )

        cls._dispatch_table = cls._build_dispatch_table()

    @classmethod
    def _build_dispatch_table(cls) -> dict[str, tuple[str, type]]:
        """Scan for @projects methods and build dispatch table."""
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

    def dispatch(self, event_any: Any) -> types.Projection:
        """Dispatch event to matching @projects method.

        Args:
            event_any: Packed event as google.protobuf.Any

        Returns:
            Projection result, or empty Projection if no handler matches.
        """
        type_url = event_any.type_url

        for suffix, (method_name, event_type) in self._dispatch_table.items():
            if type_url.endswith(suffix):
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
        projector = cls()
        last_projection = types.Projection()

        for page in source.pages:
            if page.HasField("event"):
                result = projector.dispatch(page.event)
                if result.projector:  # Non-empty projection
                    last_projection = result

        return last_projection

    @classmethod
    def descriptor(cls) -> Descriptor:
        """Build component descriptor for topology discovery."""
        # Get list of domains (support both single and multi-domain)
        domains = cls.input_domains if cls.input_domains else [cls.input_domain]
        types_list = list(cls._dispatch_table.keys())

        return Descriptor(
            name=cls.name,
            component_type="projector",
            inputs=[TargetDesc(domain=d, types=types_list) for d in domains],
        )
