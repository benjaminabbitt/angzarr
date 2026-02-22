"""Base ProcessManager class for stateful cross-domain orchestration.

Process managers coordinate long-running workflows across multiple aggregates.
They maintain their own event-sourced state (keyed by correlation_id) and
react to events from multiple domains.

Two-phase protocol support:
    1. Prepare: Declare additional destinations needed (via @prepares)
    2. Handle: Produce commands and process events (via @reacts_to)

Example usage:
    from angzarr_client import ProcessManager, prepares, reacts_to

    @dataclass
    class OrderWorkflowState:
        order_id: str = ""
        inventory_reserved: bool = False
        payment_received: bool = False

    class OrderWorkflowPM(ProcessManager[OrderWorkflowState]):
        name = "order-workflow"

        def _create_empty_state(self) -> OrderWorkflowState:
            return OrderWorkflowState()

        def _apply_event(self, state, event_any):
            if event_any.type_url.endswith("OrderCreated"):
                ...

        @prepares(OrderCreated)
        def prepare_order(self, event: OrderCreated) -> list[Cover]:
            return [Cover(domain="inventory", root=...)]

        @reacts_to(OrderCreated, input_domain="order")
        def on_order_created(
            self, event: OrderCreated, destinations: list[EventBook]
        ) -> ReserveInventory:
            return ReserveInventory(...)

        @reacts_to(InventoryReserved, input_domain="inventory")
        def on_inventory_reserved(self, event: InventoryReserved) -> ProcessPayment:
            return ProcessPayment(...)
"""

from __future__ import annotations

import inspect
from abc import ABC, abstractmethod
from typing import Generic, TypeVar

from google.protobuf.any_pb2 import Any

from .proto.angzarr import aggregate_pb2 as aggregate
from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import types_pb2 as types
from .compensation import RejectionHandlerResponse
from .router import _pack_any, prepares, reacts_to, rejected

# Re-export decorators
__all__ = ["ProcessManager", "prepares", "reacts_to", "rejected"]

StateT = TypeVar("StateT")


class ProcessManager(Generic[StateT], ABC):
    """Base class for stateful process managers.

    Process managers are event-sourced aggregates that:
    - React to events from multiple domains
    - Maintain state keyed by correlation_id
    - Produce commands to coordinate workflows
    - Handle rejection/compensation via @rejected handlers
    - Support two-phase protocol with @prepares for destination declaration

    Subclasses must:
    - Set `name` class attribute
    - Implement `_create_empty_state() -> StateT`
    - Implement `_apply_event(state: StateT, event_any: Any) -> None`
    - Decorate event handlers with `@reacts_to(EventType, input_domain="...")`
    - Optionally decorate prepare handlers with `@prepares(EventType)`
    - Optionally decorate rejection handlers with `@rejected(domain, command)`

    Usage:
        class OrderWorkflowPM(ProcessManager[OrderWorkflowState]):
            name = "order-workflow"

            def _create_empty_state(self) -> OrderWorkflowState:
                return OrderWorkflowState()

            def _apply_event(self, state, event_any):
                ...

            @prepares(OrderCreated)
            def prepare_order(self, event: OrderCreated) -> list[Cover]:
                return [Cover(domain="inventory", root=...)]

            @reacts_to(OrderCreated, input_domain="order")
            def on_order_created(
                self, event: OrderCreated, destinations: list[EventBook]
            ) -> ReserveInventory:
                ...

            @rejected(domain="inventory", command="ReserveInventory")
            def handle_reserve_rejected(self, revoke_cmd) -> WorkflowFailed:
                return WorkflowFailed(reason=revoke_cmd.rejection_reason)
    """

    name: str
    _dispatch_table: dict[
        str, tuple[str, type, str, str]
    ] = {}  # suffix -> (method, type, input_domain, output_domain)
    _prepare_table: dict[str, tuple[str, type]] = {}  # suffix -> (method, type)
    _input_domains: dict[str, list[str]] = {}  # domain -> [event types]
    _rejection_table: dict[str, str] = {}  # "domain/command" -> method_name

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Validate required class attributes
        if not getattr(cls, "name", None):
            raise TypeError(f"{cls.__name__} must define 'name' class attribute")

        cls._dispatch_table = {}
        cls._prepare_table = {}
        cls._input_domains = {}
        cls._rejection_table = {}
        cls._build_dispatch_table()
        cls._build_prepare_table()
        cls._build_rejection_table()

    @classmethod
    def _build_dispatch_table(cls):
        """Scan for @reacts_to methods and build dispatch table."""
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if callable(attr) and getattr(attr, "_is_handler", False):
                event_type = attr._event_type
                input_domain = attr._input_domain
                output_domain = attr._output_domain
                suffix = event_type.__name__

                if suffix in cls._dispatch_table:
                    raise TypeError(f"{cls.__name__}: duplicate handler for {suffix}")

                cls._dispatch_table[suffix] = (
                    attr_name,
                    event_type,
                    input_domain,
                    output_domain,
                )

                # Track input domains for descriptor
                if input_domain:
                    if input_domain not in cls._input_domains:
                        cls._input_domains[input_domain] = []
                    cls._input_domains[input_domain].append(suffix)

    @classmethod
    def _build_prepare_table(cls):
        """Scan for @prepares methods and build prepare table."""
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if callable(attr) and getattr(attr, "_is_prepare_handler", False):
                event_type = attr._event_type
                suffix = event_type.__name__
                if suffix in cls._prepare_table:
                    raise TypeError(
                        f"{cls.__name__}: duplicate prepare handler for {suffix}"
                    )
                cls._prepare_table[suffix] = (attr_name, event_type)

    @classmethod
    def _build_rejection_table(cls):
        """Scan for @rejected methods and build rejection dispatch table."""
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if callable(attr) and getattr(attr, "_is_rejection_handler", False):
                domain = attr._rejection_domain
                command = attr._rejection_command
                key = f"{domain}/{command}"
                if key in cls._rejection_table:
                    raise TypeError(
                        f"{cls.__name__}: duplicate rejection handler for {key}"
                    )
                cls._rejection_table[key] = attr_name

    def __init__(self, process_state: types.EventBook = None):
        """Initialize process manager with optional prior state.

        Args:
            process_state: Existing event book for this correlation_id.
        """
        if process_state is None:
            process_state = types.EventBook()
        self._event_book = process_state
        self._state: StateT = None
        self._new_events: list[Any] = []

    def _get_state(self) -> StateT:
        """Get current state, rebuilding from events if needed."""
        if self._state is None:
            self._state = self._rebuild()
        return self._state

    def _rebuild(self) -> StateT:
        """Rebuild state from event book."""
        state = self._create_empty_state()
        for page in self._event_book.pages:
            if page.event:
                self._apply_event(state, page.event)
        return state

    def prepare(self, event_any: Any) -> list[types.Cover]:
        """Prepare destinations for an event.

        Dispatches to @prepares decorated method if one exists.

        Args:
            event_any: Packed event as google.protobuf.Any

        Returns:
            List of Covers identifying destination aggregates.
        """
        type_url = event_any.type_url

        for suffix, (method_name, event_type) in self._prepare_table.items():
            if type_url.endswith(suffix):
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
        """Dispatch event to matching handler.

        Args:
            event_any: Packed event as google.protobuf.Any
            root: Source aggregate root
            correlation_id: Correlation ID for the workflow
            destinations: Optional list of destination EventBooks

        Returns:
            List of CommandBooks to send.
        """
        type_url = event_any.type_url

        for suffix, (
            method_name,
            event_type,
            _,
            output_domain,
        ) in self._dispatch_table.items():
            if type_url.endswith(suffix):
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
                return self._pack_commands(result, output_domain, root, correlation_id)

        return []

    def _pack_commands(
        self,
        result,
        output_domain: str = None,
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
            # Use handler's output_domain if specified
            domain = output_domain or ""
            cover = types.Cover(
                domain=domain,
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

    def _apply_and_record(self, event) -> None:
        """Pack event, apply to cached state, record for persistence."""
        event_any = _pack_any(event)

        # Apply directly to cached state
        if self._state is not None:
            self._apply_event(self._state, event_any)

        self._new_events.append(event_any)

    def process_events(self) -> types.EventBook:
        """Return new process events for persistence."""
        pages = [types.EventPage(event=e) for e in self._new_events]
        return types.EventBook(pages=pages)

    @classmethod
    def prepare_destinations(
        cls,
        trigger: types.EventBook,
        process_state: types.EventBook,
    ) -> list[types.Cover]:
        """Phase 1: Declare additional destinations needed.

        Args:
            trigger: The triggering event book.
            process_state: Current process manager state.

        Returns:
            List of Covers identifying destination aggregates to fetch.
        """
        pm = cls(process_state)
        destinations: list[types.Cover] = []

        for page in trigger.pages:
            if page.HasField("event"):
                destinations.extend(pm.prepare(page.event))

        return destinations

    @classmethod
    def handle(
        cls,
        trigger: types.EventBook,
        process_state: types.EventBook,
        destinations: list[types.EventBook] = None,
    ) -> tuple[list[types.CommandBook], types.EventBook]:
        """Phase 2: Handle a trigger event with current process state.

        Args:
            trigger: The triggering event book.
            process_state: Current process manager state.
            destinations: Additional destination states (from prepare phase).

        Returns:
            Tuple of (commands to send, new process events).
        """
        pm = cls(process_state)
        root = trigger.cover.root.value if trigger.HasField("cover") else None
        correlation_id = (
            trigger.cover.correlation_id if trigger.HasField("cover") else ""
        )

        commands = []
        for page in trigger.pages:
            if page.HasField("event"):
                commands.extend(
                    pm.dispatch(page.event, root, correlation_id, destinations)
                )

        return commands, pm.process_events()

    def handle_revocation(
        self,
        notification: types.Notification,
    ) -> RejectionHandlerResponse:
        """Handle a rejection notification for commands this PM issued.

        Called when a command produced by this PM is rejected by the target aggregate.
        Dispatches to @rejected decorated methods based on target domain
        and rejected command type.

        If no matching @rejected handler is found, returns empty response
        (framework handles default behavior).

        Args:
            notification: Notification containing RejectionNotification payload.

        Returns:
            RejectionHandlerResponse with optional events (compensation)
            and/or notification (upstream propagation).

        Usage:
            @rejected(domain="inventory", command="ReserveInventory")
            def handle_reserve_rejected(self, notification) -> RejectionHandlerResponse:
                ctx = CompensationContext.from_notification(notification)
                self._apply_and_record(WorkflowFailed(reason=ctx.rejection_reason))
                return RejectionHandlerResponse(events=self.process_events())
        """
        # Unpack rejection details from notification payload
        rejection = types.RejectionNotification()
        if notification.HasField("payload"):
            notification.payload.Unpack(rejection)

        # Extract domain and command type from rejected_command
        domain = ""
        command_suffix = ""

        if rejection.HasField("rejected_command") and rejection.rejected_command.pages:
            rejected_cmd = rejection.rejected_command
            if rejected_cmd.HasField("cover"):
                domain = rejected_cmd.cover.domain
            if rejected_cmd.pages[0].HasField("command"):
                type_url = rejected_cmd.pages[0].command.type_url
                # Extract suffix (e.g., "ReserveInventory" from "type.googleapis.com/.../ReserveInventory")
                command_suffix = (
                    type_url.rsplit("/", 1)[-1] if "/" in type_url else type_url
                )

        # Dispatch to @rejected handler if found (use suffix matching like regular dispatch)
        for key, method_name in self._rejection_table.items():
            expected_domain, expected_command = key.split("/", 1)
            if domain == expected_domain and command_suffix.endswith(expected_command):
                # Ensure state is built before calling handler
                _ = self._get_state()
                # Call the handler
                result = getattr(self, method_name)(notification)

                # Handler may return RejectionHandlerResponse directly
                if isinstance(result, RejectionHandlerResponse):
                    return result

                # Handler may return None or a single event
                process_events = self.process_events()
                return RejectionHandlerResponse(
                    events=process_events if process_events.pages else None,
                )

        # Default: no handler, return empty response
        return RejectionHandlerResponse()

    @property
    def state(self) -> StateT:
        """Get current state (convenience property for _get_state)."""
        return self._get_state()

    @abstractmethod
    def _create_empty_state(self) -> StateT:
        """Create an empty state instance. Must be implemented by subclasses."""
        ...

    @abstractmethod
    def _apply_event(self, state: StateT, event_any: Any) -> None:
        """Apply a single event to state. Must be implemented by subclasses."""
        ...
