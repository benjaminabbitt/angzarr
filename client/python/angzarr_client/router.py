"""Unified Router module for aggregates, sagas, process managers, and projectors.

This module provides:

1. Router (unified core):
   - Routes events, commands, and notifications by type_url
   - All specialized routers delegate to this for dispatch

2. Fluent Routers (functional handler pattern, sibling classes):
   - SingleFluentRouter(name, domain).on() - single-domain, domain in constructor
   - FluentRouter(name).domain(d).on() - multi-domain, domain via method
   - Both extend _FluentRouterBase (internal) for shared dispatch logic

3. Protocol-based Routers (wrap handler objects):
   - CommandHandlerRouter, ProcessManagerRouter, ProjectorRouter
   - Wrap handler protocol implementations

4. OORouter (class-based with decorators):
   - Add @domain decorated classes via .add(cls)
   - Each class handles a single domain
   - Multi-domain: add multiple classes

5. Decorators:
   - @domain("name") - class decorator marking handler class domain
   - @handles(Event) - method decorator for event handlers
   - @prepares(Event) - method decorator for prepare handlers
   - @rejected(domain, command) - method decorator for rejection handlers

Note: aggregate.py has its own @handles decorator for command handlers.
Import from the appropriate module for your use case:
   - from angzarr_client import handles  # aggregate command handlers
   - from angzarr_client.saga import handles  # saga event handlers
   - from angzarr_client.projector import handles  # projector event handlers
   - from angzarr_client.process_manager import handles  # PM event handlers

Usage (Fluent builder - single domain):
    router = (
        SingleFluentRouter("saga-table-hand", "table")
        .prepare(HandStarted, prepare_hand)
        .on(HandStarted, handle_hand)
    )

Usage (Fluent builder - multi-domain):
    router = (
        FluentRouter("pmg-order-workflow")
        .domain("order").on(OrderCreated, handle_order)
        .domain("inventory").on(StockReserved, handle_stock)
    )

Usage (Protocol-based):
    router = CommandHandlerRouter("player", "player", PlayerHandler())

Usage (OO with decorators):
    class TableHandSaga(Saga, domain="table"):
        @handles(HandStarted, prepare=True)
        def prepare_hand(self, event): ...

        @handles(HandStarted)
        def handle_started(self, event, destinations): ...

    router = OORouter("saga-table-hand").add(TableHandSaga)
"""

from __future__ import annotations

import inspect
import typing
from collections.abc import Callable
from functools import wraps
from typing import TYPE_CHECKING, Generic, TypeVar
from typing import Any as TypingAny

from google.protobuf import any_pb2

from .handler_protocols import (
    CommandHandlerDomainHandler,
    ProcessManagerDomainHandler,
    ProjectorDomainHandler,
)
from .helpers import TYPE_URL_PREFIX
from .proto.angzarr import command_handler_pb2 as ch_pb2
from .proto.angzarr import process_manager_pb2 as pm
from .proto.angzarr import saga_pb2
from .proto.angzarr import types_pb2 as types

if TYPE_CHECKING:
    pass

S = TypeVar("S")

# Type URL for Notification messages
NOTIFICATION_TYPE_URL = TYPE_URL_PREFIX + "angzarr.Notification"

# Error message constants
ERRMSG_UNKNOWN_COMMAND = "Unknown command type"
ERRMSG_NO_COMMAND_PAGES = "No command pages"


# ============================================================================
# Helper Functions
# ============================================================================


def next_sequence(events: types.EventBook | None) -> int:
    """Compute the next event sequence number from prior events."""
    if events is None or not events.pages:
        return 0
    return len(events.pages)


# Alias for internal use
_next_sequence = next_sequence


def _pack_any(event) -> any_pb2.Any:
    """Pack a protobuf message into Any."""
    event_any = any_pb2.Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")
    return event_any


def _pack_events(result, start_seq: int = 0) -> types.EventBook:
    """Pack event(s) into an EventBook.

    Args:
        result: Single event, tuple of events, or None.
        start_seq: Starting sequence number for events.

    Returns:
        EventBook containing packed events with proper sequences.
    """
    pages = []

    if result is None:
        pass
    elif isinstance(result, tuple):
        for i, event in enumerate(result):
            pages.append(
                types.EventPage(
                    header=types.PageHeader(sequence=start_seq + i),
                    event=_pack_any(event),
                )
            )
    else:
        pages.append(
            types.EventPage(
                header=types.PageHeader(sequence=start_seq), event=_pack_any(result)
            )
        )

    return types.EventBook(pages=pages)


def _extract_rejection_key(
    rejection: types.RejectionNotification,
) -> tuple[str, str]:
    """Extract domain and command type name from a RejectionNotification."""
    domain = ""
    command_type_name = ""

    if rejection.HasField("rejected_command") and rejection.rejected_command.pages:
        rejected_cmd = rejection.rejected_command
        if rejected_cmd.HasField("cover"):
            domain = rejected_cmd.cover.domain
        if rejected_cmd.pages[0].HasField("command"):
            cmd_type_url = rejected_cmd.pages[0].command.type_url
            # Extract type name after the prefix
            if cmd_type_url.startswith(TYPE_URL_PREFIX):
                command_type_name = cmd_type_url[len(TYPE_URL_PREFIX) :]
            else:
                command_type_name = cmd_type_url

    return domain, command_type_name


# ============================================================================
# Decorator Validation
# ============================================================================


def validate_command_handler(
    func: Callable,
    command_type: type,
    cmd_param_index: int,
    decorator_name: str,
) -> str:
    """Validate a command handler's signature.

    Shared validation logic for @handles and other decorators.

    Args:
        func: The function/method being decorated.
        command_type: Expected command type from decorator argument.
        cmd_param_index: Index of the cmd parameter (0 for functions, 1 for methods).
        decorator_name: Name of decorator for error messages.

    Returns:
        The name of the cmd parameter.

    Raises:
        TypeError: If validation fails.
    """
    hints = typing.get_type_hints(func)
    sig = inspect.signature(func)
    params = list(sig.parameters.keys())

    min_params = cmd_param_index + 1
    if len(params) < min_params:
        raise TypeError(f"{func.__name__}: must have cmd parameter")

    cmd_param = params[cmd_param_index]
    if cmd_param not in hints:
        raise TypeError(f"{func.__name__}: missing type hint for '{cmd_param}'")

    hint_type = hints[cmd_param]
    if hint_type != command_type:
        raise TypeError(
            f"{func.__name__}: @{decorator_name}({command_type.__name__}) "
            f"doesn't match type hint {hint_type.__name__}"
        )

    return cmd_param


# ============================================================================
# @domain class decorator
# ============================================================================


def domain(name: str):
    """Class decorator to mark handler class domain.

    All @handles methods in this class will be registered to this domain.
    Use with OORouter or as base for Saga/ProcessManager/Projector classes.

    Args:
        name: The domain name (e.g., "order", "inventory").

    Example:
        @domain("order")
        class OrderHandlers:
            @handles(OrderCreated)
            def on_order(self, event: OrderCreated):
                ...

        router = OORouter("my-component").add(OrderHandlers)
    """

    def decorator(cls):
        cls._domain = name
        return cls

    return decorator


# ============================================================================
# @output_domain decorator (class or method)
# ============================================================================


def output_domain(name: str):
    """Decorator to mark output domain for sagas or PM methods.

    On class (Saga): sets the domain commands are sent to.
    On method (ProcessManager): sets output domain for that handler.

    Args:
        name: The output domain name (e.g., "hand", "inventory").

    Example (Saga class):
        @domain("table")
        @output_domain("hand")
        class TableHandSaga(Saga):
            name = "saga-table-hand"

            @handles(HandStarted)
            def handle_started(self, event): ...

    Example (ProcessManager method):
        class OrderWorkflowPM(ProcessManager[State]):
            @output_domain("inventory")
            @handles(OrderCreated, input_domain="order")
            def on_order(self, event, state): ...
    """

    def decorator(target):
        target._output_domain = name
        return target

    return decorator


# ============================================================================
# @handles decorator (unified handler decorator)
# ============================================================================


def handles(
    msg_type: type,
    *,
    prepare: bool = False,
    input_domain: str = None,
    output_domain: str = None,
):
    """Unified decorator for handler methods.

    Registers the method as a handler for the given message type.
    Works for sagas, process managers, projectors, and command handlers.

    Domain resolution:
    - Primary: use @domain class decorator on the containing class
    - Override: specify input_domain parameter (for multi-domain ProcessManagers)

    Args:
        msg_type: The protobuf message class to handle.
        prepare: If True, this is a prepare handler (destination declaration).
        input_domain: Optional explicit input domain (overrides @domain decorator).
            Use for ProcessManagers that listen to multiple domains.
        output_domain: Optional explicit output domain for commands.
            Use for ProcessManagers that emit to specific domains.

    Example (Saga - single domain via @domain):
        @domain("order")
        class OrderFulfillmentSaga(Saga):
            @handles(OrderCompleted)
            def handle_completed(self, event: OrderCompleted) -> CreateShipment:
                return CreateShipment(order_id=event.order_id)

    Example (ProcessManager - multi-domain via input_domain):
        class OrderWorkflowPM(ProcessManager[State]):
            @handles(OrderCreated, input_domain="order", output_domain="inventory")
            def on_order(self, event: OrderCreated) -> ReserveStock:
                return ReserveStock(...)

            @handles(StockReserved, input_domain="inventory", output_domain="payment")
            def on_stock(self, event: StockReserved) -> ProcessPayment:
                return ProcessPayment(...)

    Example (prepare handler):
        @handles(HandStarted, prepare=True)
        def prepare_hand(self, event: HandStarted) -> list[Cover]:
            return [Cover(domain="hand", root=UUID(value=event.hand_root))]

    Raises:
        TypeError: If type hint is missing or doesn't match msg_type.
    """

    def decorator(method: Callable) -> Callable:
        # Validate at decoration time (msg is at index 1 after self)
        validate_command_handler(
            method, msg_type, cmd_param_index=1, decorator_name="handles"
        )

        @wraps(method)
        def wrapper(self, *args, **kwargs):
            return method(self, *args, **kwargs)

        if prepare:
            wrapper._is_prepare_handler = True
        else:
            wrapper._is_handler = True

        wrapper._event_type = msg_type
        wrapper._command_type = msg_type  # For aggregate compatibility
        wrapper._input_domain = input_domain
        wrapper._output_domain = output_domain
        return wrapper

    return decorator


def prepares(event_type: type):
    """Decorator for prepare handler methods (destination declaration phase).

    Registers the method as a prepare handler for the given event type.
    Used in sagas and process managers to declare which destination
    aggregates are needed before the main handler executes.

    Args:
        event_type: The protobuf event class to handle.

    Example:
        @prepares(HandStarted)
        def prepare_hand(self, event: HandStarted) -> list[Cover]:
            return [Cover(domain="hand", root=UUID(value=event.hand_root))]

    Raises:
        TypeError: If type hint is missing or doesn't match event_type.
    """
    return handles(event_type, prepare=True)


# ============================================================================
# @command_handler decorator for functional command handlers
# ============================================================================


def command_handler(command_type: type):
    """Decorator for functional command handler functions.

    Marks a function as a command handler for the given command type.
    Used with CommandHandlerRouter for the functional/fluent handler pattern.

    The decorated function must have signature:
        (cmd: CommandType, state: StateType, seq: int) -> EventType

    Args:
        command_type: The protobuf command class to handle.

    Example:
        @command_handler(RegisterPlayer)
        def handle_register_player(
            cmd: RegisterPlayer, state: PlayerState, seq: int
        ) -> PlayerRegistered:
            if state.exists:
                raise CommandRejectedError("Player already exists")
            return PlayerRegistered(name=cmd.name, ...)

    Raises:
        TypeError: If type hint is missing or doesn't match command_type.
    """

    def decorator(func: Callable) -> Callable:
        # Validate at decoration time (cmd is at index 0 for functions)
        validate_command_handler(
            func, command_type, cmd_param_index=0, decorator_name="command_handler"
        )

        @wraps(func)
        def wrapper(*args, **kwargs):
            return func(*args, **kwargs)

        wrapper._is_handler = True
        wrapper._command_type = command_type
        return wrapper

    return decorator


# ============================================================================
# @rejected decorator for compensation handlers
# ============================================================================


def rejected(*, domain: str, command: str):
    """Decorator for rejection handler methods on Aggregate or ProcessManager.

    Registers the method as a handler for when a specific command is rejected.
    The method is called when a saga/PM command targeting the specified domain
    and command type is rejected by the target aggregate.

    Args:
        domain: The target domain of the rejected command.
        command: The type name of the rejected command.

    Example (Aggregate):
        @rejected(domain="payment", command="ProcessPayment")
        def handle_payment_rejected(self, notification: Notification) -> FundsReleased:
            rejection = RejectionNotification()
            notification.payload.Unpack(rejection)
            return FundsReleased(
                player_root=self.state.player_root,
                amount=self.state.reserved_amount,
                reason=f"Payment failed: {rejection.rejection_reason}",
            )

    Example (ProcessManager):
        @rejected(domain="inventory", command="ReserveInventory")
        def handle_reserve_rejected(self, notification: Notification) -> WorkflowFailed:
            rejection = RejectionNotification()
            notification.payload.Unpack(rejection)
            return WorkflowFailed(reason=rejection.rejection_reason)
    """

    def decorator(method: Callable) -> Callable:
        @wraps(method)
        def wrapper(self, *args, **kwargs):
            result = method(self, *args, **kwargs)
            # For aggregates, auto-apply and record the event
            if hasattr(self, "_apply_and_record") and result is not None:
                if isinstance(result, tuple):
                    for event in result:
                        self._apply_and_record(event)
                else:
                    self._apply_and_record(result)
            return result

        wrapper._is_rejection_handler = True
        wrapper._rejection_domain = domain
        wrapper._rejection_command = command
        return wrapper

    return decorator


# ============================================================================
# Unified Router (core routing for all message types)
# ============================================================================


class Router:
    """Unified router core - routes events, commands, and notifications by type_url.

    All specialized routers (SingleFluentRouter, CommandRouter, etc.) delegate to this
    class for the core dispatch logic. This ensures consistent type_url matching
    across all message types.

    Example:
        router = Router("my-component")
        router.register("order", OrderCreated, handle_order_created)
        router.register("order", OrderCompleted, handle_order_completed, is_prepare=True)

        # Dispatch to matching handler
        result = router.dispatch("order", event_any, *args)
    """

    def __init__(self, name: str) -> None:
        """Create a new unified router.

        Args:
            name: The component name (e.g., "saga-table-hand").
        """
        self._name = name
        # domain -> {type_suffix -> (msg_type, handler)}
        self._handlers: dict[str, dict[str, tuple[type, Callable]]] = {}
        self._prepare_handlers: dict[str, dict[str, tuple[type, Callable]]] = {}
        # "domain/command" -> (handler, domain, command)
        self._rejection_handlers: dict[str, tuple[Callable, str, str]] = {}

    @property
    def name(self) -> str:
        """Get the router name."""
        return self._name

    def domains(self) -> list[str]:
        """Get list of registered domains."""
        return list(set(self._handlers.keys()) | set(self._prepare_handlers.keys()))

    def register(
        self,
        domain: str,
        msg_type: type,
        handler: Callable,
        *,
        is_prepare: bool = False,
    ) -> None:
        """Register a handler for a message type in a domain.

        Args:
            domain: The domain name.
            msg_type: The protobuf message class.
            handler: The handler function.
            is_prepare: If True, register as prepare handler (two-phase protocol).
        """
        handlers = self._prepare_handlers if is_prepare else self._handlers
        if domain not in handlers:
            handlers[domain] = {}
        type_suffix = msg_type.DESCRIPTOR.full_name
        handlers[domain][type_suffix] = (msg_type, handler)

    def types_for_domain(self, domain: str) -> list[str]:
        """Get registered type suffixes for a domain.

        Args:
            domain: The domain name.

        Returns:
            List of type suffixes (e.g., ["OrderCreated", "OrderCompleted"]).
        """
        domain_handlers = self._handlers.get(domain, {})
        return [name.rsplit(".", 1)[-1] for name in domain_handlers.keys()]

    def dispatch(
        self,
        domain: str,
        msg_any: any_pb2.Any,
        *args,
        **kwargs,
    ) -> TypingAny:
        """Dispatch a message to its handler.

        Works for events, commands, and notifications. Matches by type_url suffix.

        Args:
            domain: The domain to dispatch within.
            msg_any: The message wrapped in Any.
            *args, **kwargs: Additional arguments passed to the handler.

        Returns:
            Handler result, or None if no handler matched.
        """
        type_suffix = self._extract_type_suffix(msg_any.type_url)
        domain_handlers = self._handlers.get(domain, {})

        for full_name, (msg_type, handler) in domain_handlers.items():
            if full_name.endswith(type_suffix) or type_suffix in full_name:
                return handler(msg_any, *args, **kwargs)
        return None

    def dispatch_prepare(
        self,
        domain: str,
        msg_any: any_pb2.Any,
        *args,
        **kwargs,
    ) -> TypingAny:
        """Dispatch prepare phase for two-phase protocol.

        Args:
            domain: The domain to dispatch within.
            msg_any: The message wrapped in Any.
            *args, **kwargs: Additional arguments passed to the handler.

        Returns:
            Handler result (typically list[Cover]), or None if no handler matched.
        """
        type_suffix = self._extract_type_suffix(msg_any.type_url)
        domain_handlers = self._prepare_handlers.get(domain, {})

        for full_name, (msg_type, handler) in domain_handlers.items():
            if full_name.endswith(type_suffix) or type_suffix in full_name:
                return handler(msg_any, *args, **kwargs)
        return None

    @staticmethod
    def _extract_type_suffix(type_url: str) -> str:
        """Extract type name suffix from type_url.

        Args:
            type_url: Full type URL (e.g., "type.googleapis.com/angzarr.OrderCreated")

        Returns:
            Type suffix (e.g., "OrderCreated")
        """
        return type_url.rsplit(".", 1)[-1] if type_url else ""

    def register_rejection(
        self,
        domain: str,
        command: str,
        handler: Callable,
    ) -> None:
        """Register a rejection handler for a domain/command pair.

        Args:
            domain: The target domain of the rejected command.
            command: The command type name (e.g., "CreateHand").
            handler: The handler function.
        """
        key = f"{domain}/{command}"
        self._rejection_handlers[key] = (handler, domain, command)

    def dispatch_rejection(
        self,
        notification: types.Notification,
        *args,
        **kwargs,
    ) -> TypingAny:
        """Dispatch rejection notification to matching handler.

        Uses suffix matching on command type (like existing code).

        Args:
            notification: The Notification message.
            *args, **kwargs: Additional arguments passed to the handler.

        Returns:
            Handler result, or None if no handler matched.
        """
        rejection = types.RejectionNotification()
        if notification.HasField("payload"):
            notification.payload.Unpack(rejection)

        domain, command_suffix = _extract_rejection_key(rejection)

        for key, (
            handler,
            expected_domain,
            expected_command,
        ) in self._rejection_handlers.items():
            if domain == expected_domain and command_suffix.endswith(expected_command):
                return handler(notification, *args, **kwargs)
        return None

    # =========================================================================
    # Static helpers (shared across all router types)
    # =========================================================================

    @staticmethod
    def next_sequence(event_book: types.EventBook | None) -> int:
        """Compute next sequence number from an event book.

        Args:
            event_book: The event book (may be None).

        Returns:
            Next sequence number (0 if event_book is None or empty).
        """
        return _next_sequence(event_book)

    @staticmethod
    def pack_any(message) -> any_pb2.Any:
        """Pack any protobuf message into Any.

        Args:
            message: The protobuf message to pack.

        Returns:
            Any containing the packed message.
        """
        return _pack_any(message)


# ============================================================================
# Simplified Router Hierarchy
# ============================================================================
#
# Design:
#   Router (core type_url dispatch)
#   ├── OORouter(name).add(cls)                      # @domain decorated classes
#   └── _FluentRouterBase (internal)                 # shared fluent dispatch logic
#       ├── SingleFluentRouter(name, domain).on(...) # single-domain
#       └── FluentRouter(name).domain(d).on(...)     # multi-domain
#
# All routing logic lives in Router. The higher-level classes just provide
# different APIs for registering handlers.
#
# _FluentRouterBase contains shared dispatch logic. SingleFluentRouter and
# FluentRouter are siblings (not parent-child) to prevent accidental
# .domain() calls on single-domain routers.
#
# OORouter handles both single and multi-domain cases via .add():
#   - Single domain: add one @domain decorated class
#   - Multi-domain: add multiple @domain decorated classes
# ============================================================================


class _FluentRouterBase:
    """Internal base class for fluent routers.

    Contains shared dispatch logic. Not for direct use.
    Use SingleFluentRouter or FluentRouter instead.
    """

    _router: Router
    _domains: set[str]

    @property
    def name(self) -> str:
        """Get the router name."""
        return self._router.name

    def domains(self) -> list[str]:
        """Get list of registered domains."""
        return list(self._domains)

    def types_for_domain(self, domain: str) -> list[str]:
        """Get registered type names for a domain."""
        return self._router.types_for_domain(domain)

    def subscriptions(self) -> list[tuple[str, list[str]]]:
        """Get subscriptions for this router."""
        return [(d, self.types_for_domain(d)) for d in self._domains]

    def _get_registration_domain(self) -> str:
        """Get domain to use for registering handlers. Subclasses must implement."""
        raise NotImplementedError

    def _get_dispatch_domain(self, source: types.EventBook) -> str:
        """Get domain to dispatch to. Subclasses must implement."""
        raise NotImplementedError

    def _register_prepare(self, msg_type: type, handler: Callable) -> None:
        """Register a prepare handler."""
        domain = self._get_registration_domain()
        self._router.register(domain, msg_type, handler, is_prepare=True)

    def _register_handler(self, msg_type: type, handler: Callable) -> None:
        """Register an event handler."""
        domain = self._get_registration_domain()
        self._router.register(domain, msg_type, handler)

    def _register_rejection(self, domain: str, command: str, handler: Callable) -> None:
        """Register a rejection handler."""
        self._router.register_rejection(domain, command, handler)

    def prepare_destinations(self, source: types.EventBook | None) -> list[types.Cover]:
        """Execute prepare phase for source events."""
        if source is None or not source.pages:
            return []

        event_page = source.pages[-1]
        if not event_page.HasField("event"):
            return []

        event_any = event_page.event
        root = source.cover.root if source.HasField("cover") else None
        domain = self._get_dispatch_domain(source)

        result = self._router.dispatch_prepare(domain, event_any, root)
        return result if result is not None else []

    def dispatch(
        self, source: types.EventBook, destinations: list[types.EventBook] | None = None
    ) -> saga_pb2.SagaResponse:
        """Handle source events and produce commands.

        Returns:
            SagaResponse containing commands and events.
        """
        if not source.pages:
            return saga_pb2.SagaResponse()

        event_page = source.pages[-1]
        event_any = self._get_event_from_page(event_page)
        if event_any is None:
            return saga_pb2.SagaResponse()

        # Check if this is a Notification (rejection)
        if event_any.type_url.endswith("Notification"):
            commands = self.dispatch_rejection(source, destinations or [])
            return saga_pb2.SagaResponse(commands=commands)

        root = source.cover.root if source.HasField("cover") else None
        correlation_id = source.cover.correlation_id if source.HasField("cover") else ""
        domain = self._get_dispatch_domain(source)

        result = self._router.dispatch(
            domain, event_any, root, correlation_id, destinations or []
        )
        commands = result if result is not None else []
        return saga_pb2.SagaResponse(commands=commands)

    def _get_event_from_page(self, page: types.EventPage) -> any_pb2.Any | None:
        """Extract event from EventPage, handling both old and new proto formats."""
        # Try old format: direct event field
        if page.HasField("event"):
            return page.event
        # Try new format: event in payload oneof
        if hasattr(page, "payload") and page.WhichOneof("payload") == "event":
            return page.event
        # Try getter method
        if hasattr(page, "GetEvent"):
            evt = page.GetEvent()
            if evt and evt.type_url:
                return evt
        return None

    def dispatch_rejection(
        self, source: types.EventBook, destinations: list[types.EventBook]
    ) -> list[types.CommandBook]:
        """Dispatch rejection notification to handler.

        Args:
            source: The source EventBook containing the Notification.
            destinations: EventBooks for destinations (for compensating commands).

        Returns:
            List of CommandBooks (compensating commands), or empty list if no handler.
        """
        if not source.pages:
            return []

        event_page = source.pages[-1]
        if not event_page.HasField("event"):
            return []

        event_any = event_page.event
        if not event_any.type_url.endswith("Notification"):
            return []

        notification = types.Notification()
        event_any.Unpack(notification)

        root = source.cover.root if source.HasField("cover") else None
        correlation_id = source.cover.correlation_id if source.HasField("cover") else ""

        result = self._router.dispatch_rejection(
            notification, root, correlation_id, destinations
        )
        return result if result is not None else []

    next_sequence = staticmethod(Router.next_sequence)
    pack_any = staticmethod(Router.pack_any)


class SingleFluentRouter(_FluentRouterBase):
    """Single-domain fluent builder API.

    Domain is fixed at construction - no .domain() method.
    Use .prepare() and .on() to register handlers.

    For multi-domain support, use FluentRouter instead (sibling class).

    Example:
        router = (
            SingleFluentRouter("saga-order-fulfillment", "order")
            .prepare(OrderCompleted, prepare_completed)
            .on(OrderCompleted, handle_completed)
        )
    """

    def __init__(self, name: str, domain: str) -> None:
        """Create a single-domain fluent router.

        Args:
            name: The component name.
            domain: THE input domain for this router (fixed at construction).
        """
        self._router = Router(name)
        self._domain = domain
        self._domains: set[str] = {domain}

    @property
    def input_domain(self) -> str:
        """Get THE input domain for this router."""
        return self._domain

    def event_types(self) -> list[str]:
        """Get registered event type names."""
        return self._router.types_for_domain(self._domain)

    def _get_registration_domain(self) -> str:
        """Get domain for registration (fixed at construction)."""
        return self._domain

    def _get_dispatch_domain(self, source: types.EventBook) -> str:
        """Get domain for dispatch (fixed at construction)."""
        return self._domain

    def prepare(
        self,
        msg_type: type,
        handler: Callable[[any_pb2.Any, types.UUID | None], list[types.Cover]],
    ) -> SingleFluentRouter:
        """Register a prepare handler for a message type.

        Args:
            msg_type: The protobuf message class.
            handler: Function (msg_any, root) -> list[Cover].

        Returns:
            Self for fluent chaining.
        """
        self._register_prepare(msg_type, handler)
        return self

    def on(
        self,
        msg_type: type,
        handler: Callable[
            [any_pb2.Any, types.UUID | None, str, list[types.EventBook]],
            list[types.CommandBook],
        ],
    ) -> SingleFluentRouter:
        """Register a handler for a message type.

        Args:
            msg_type: The protobuf message class.
            handler: Function (msg_any, root, correlation_id, destinations) -> list[CommandBook].

        Returns:
            Self for fluent chaining.
        """
        self._register_handler(msg_type, handler)
        return self

    def on_rejected(
        self,
        domain: str,
        command: str,
        handler: Callable[
            [types.Notification, types.UUID | None, str, list[types.EventBook]],
            list[types.CommandBook],
        ],
    ) -> SingleFluentRouter:
        """Register a rejection handler for a domain/command pair.

        Called when a command emitted by this saga is rejected.

        Args:
            domain: The target domain of the rejected command.
            command: The command type name (e.g., "CreateHand").
            handler: Function (notification, root, correlation_id, destinations) -> list[CommandBook].

        Returns:
            Self for fluent chaining.

        Example:
            router = (
                SingleFluentRouter("saga-table-hand", "table")
                .prepare(HandStarted, prepare_hand)
                .on(HandStarted, handle_started)
                .on_rejected("hand", "CreateHand", handle_hand_rejected)
            )
        """
        self._register_rejection(domain, command, handler)
        return self


class FluentRouter(_FluentRouterBase):
    """Multi-domain fluent builder API.

    Use .domain() to switch between domains, then .on() to register handlers.
    Chain .domain() and .on() for multi-domain handler registration.

    For single-domain, use SingleFluentRouter instead (sibling class).

    Example:
        router = (
            FluentRouter("pmg-order-workflow")
            .domain("order")
            .on(OrderCreated, handle_order_created)
            .domain("inventory")
            .on(StockReserved, handle_stock_reserved)
        )
    """

    def __init__(self, name: str) -> None:
        """Create a multi-domain fluent router.

        Args:
            name: The component name.
        """
        self._router = Router(name)
        self._domains: set[str] = set()
        self._current_domain: str | None = None

    def domain(self, name: str) -> FluentRouter:
        """Set current domain context for subsequent registrations.

        Can be called multiple times to switch between domains.

        Args:
            name: The domain name.

        Returns:
            Self for fluent chaining.
        """
        self._domains.add(name)
        self._current_domain = name
        return self

    def _get_registration_domain(self) -> str:
        """Get current domain for registration."""
        if self._current_domain is None:
            raise ValueError("Must call domain() before registering handlers")
        return self._current_domain

    def _get_dispatch_domain(self, source: types.EventBook) -> str:
        """Get domain from source event book."""
        return source.cover.domain if source.HasField("cover") else ""

    def prepare(
        self,
        msg_type: type,
        handler: Callable[[any_pb2.Any, types.UUID | None], list[types.Cover]],
    ) -> FluentRouter:
        """Register a prepare handler for a message type.

        Args:
            msg_type: The protobuf message class.
            handler: Function (msg_any, root) -> list[Cover].

        Returns:
            Self for fluent chaining.
        """
        self._register_prepare(msg_type, handler)
        return self

    def on(
        self,
        msg_type: type,
        handler: Callable[
            [any_pb2.Any, types.UUID | None, str, list[types.EventBook]],
            list[types.CommandBook],
        ],
    ) -> FluentRouter:
        """Register a handler for a message type.

        Args:
            msg_type: The protobuf message class.
            handler: Function (msg_any, root, correlation_id, destinations) -> list[CommandBook].

        Returns:
            Self for fluent chaining.
        """
        self._register_handler(msg_type, handler)
        return self

    def on_rejected(
        self,
        domain: str,
        command: str,
        handler: Callable[
            [types.Notification, types.UUID | None, str, list[types.EventBook]],
            list[types.CommandBook],
        ],
    ) -> FluentRouter:
        """Register a rejection handler for a domain/command pair.

        Called when a command emitted by this saga/PM is rejected.

        Args:
            domain: The target domain of the rejected command.
            command: The command type name (e.g., "CreateHand").
            handler: Function (notification, root, correlation_id, destinations) -> list[CommandBook].

        Returns:
            Self for fluent chaining.

        Example:
            router = (
                FluentRouter("pmg-order-workflow")
                .domain("order").on(OrderCreated, handle_order)
                .on_rejected("inventory", "ReserveStock", handle_reserve_rejected)
            )
        """
        self._register_rejection(domain, command, handler)
        return self


class OORouter:
    """Multi-domain router with fluent class registration.

    Add @domain decorated classes via .add(cls). Each class handles
    events for a single domain; add multiple classes for multi-domain.

    Example:
        @domain("order")
        class OrderHandlers:
            @handles(OrderCreated)
            def on_order(self, event): ...

        @domain("inventory")
        class InventoryHandlers:
            @handles(StockReserved)
            def on_stock(self, event): ...

        router = (
            OORouter("pmg-order-workflow")
            .add(OrderHandlers)
            .add(InventoryHandlers)
        )
    """

    def __init__(self, name: str) -> None:
        """Create a multi-domain OO router.

        Args:
            name: The component name.
        """
        self._router = Router(name)
        self._domains: set[str] = set()

    @property
    def name(self) -> str:
        """Get the router name."""
        return self._router.name

    def domains(self) -> list[str]:
        """Get list of registered domains."""
        return list(self._domains)

    def types_for_domain(self, domain: str) -> list[str]:
        """Get registered type names for a domain."""
        return self._router.types_for_domain(domain)

    def subscriptions(self) -> list[tuple[str, list[str]]]:
        """Get subscriptions (domain + types) for topology registration."""
        return [(d, self.types_for_domain(d)) for d in self._domains]

    def add(self, cls) -> OORouter:
        """Add handlers from a class with @domain or @handles(input_domain=) decorators.

        Scans cls for @handles and @prepares decorated methods.
        Domain resolution (per handler):
        1. Handler's explicit input_domain (from @handles(E, input_domain="x"))
        2. Class-level _domain (from @domain decorator)

        Args:
            cls: Class with @domain decorator and/or @handles methods with input_domain.

        Returns:
            Self for fluent chaining.

        Raises:
            ValueError: If handler has no domain (no @domain and no input_domain).
        """
        class_domain = getattr(cls, "_domain", None)
        self._scan_class(cls, class_domain)
        return self

    def _scan_class(self, cls, class_domain: str | None) -> None:
        """Scan a class for decorated methods and register handlers."""
        for attr_name in dir(cls):
            attr = getattr(cls, attr_name, None)
            if not callable(attr):
                continue

            # Check for @handles decorated methods
            if getattr(attr, "_is_handler", False):
                event_type = attr._event_type
                # Use handler's input_domain if specified, else fall back to class domain
                handler_domain = getattr(attr, "_input_domain", None) or class_domain
                if handler_domain is None:
                    raise ValueError(
                        f"{cls.__name__}.{attr_name}: must have @domain on class "
                        "or input_domain in @handles"
                    )
                self._domains.add(handler_domain)
                self._register_handler(cls, attr_name, handler_domain, event_type)

            # Check for @prepares decorated methods
            if getattr(attr, "_is_prepare_handler", False):
                event_type = attr._event_type
                handler_domain = getattr(attr, "_input_domain", None) or class_domain
                if handler_domain is None:
                    raise ValueError(
                        f"{cls.__name__}.{attr_name}: must have @domain on class "
                        "or input_domain in @handles"
                    )
                self._domains.add(handler_domain)
                self._register_prepare_handler(
                    cls, attr_name, handler_domain, event_type
                )

            # Check for @rejected decorated methods (domain is explicit, not class-level)
            if getattr(attr, "_is_rejection_handler", False):
                domain = attr._rejection_domain
                command = attr._rejection_command
                self._register_rejection_handler(cls, attr_name, domain, command)

    def _register_handler(
        self, cls, method_name: str, domain: str, msg_type: type
    ) -> None:
        """Register a handler that creates instance and calls method."""

        def handler(msg_any, root, correlation_id, destinations):
            instance = cls()
            method = getattr(instance, method_name)
            # Unpack the message
            msg = msg_type()
            msg_any.Unpack(msg)
            # Check method signature for destinations param
            sig = inspect.signature(method)
            if "destinations" in sig.parameters:
                return method(msg, destinations=destinations)
            return method(msg)

        self._router.register(domain, msg_type, handler)

    def _register_prepare_handler(
        self, cls, method_name: str, domain: str, msg_type: type
    ) -> None:
        """Register a prepare handler that creates instance and calls method."""

        def handler(msg_any, root):
            instance = cls()
            method = getattr(instance, method_name)
            msg = msg_type()
            msg_any.Unpack(msg)
            return method(msg)

        self._router.register(domain, msg_type, handler, is_prepare=True)

    def _register_rejection_handler(
        self, cls, method_name: str, domain: str, command: str
    ) -> None:
        """Register a rejection handler that creates instance and calls method."""

        def handler(notification, root, correlation_id, destinations):
            instance = cls()
            method = getattr(instance, method_name)
            # Check method signature for destinations param
            sig = inspect.signature(method)
            if "destinations" in sig.parameters:
                return method(notification, destinations=destinations)
            return method(notification)

        self._router.register_rejection(domain, command, handler)

    def prepare_destinations(self, source: types.EventBook | None) -> list[types.Cover]:
        """Execute prepare phase."""
        if source is None or not source.pages:
            return []

        domain = source.cover.domain if source.HasField("cover") else ""
        event_page = source.pages[-1]
        if not event_page.HasField("event"):
            return []

        event_any = event_page.event
        root = source.cover.root if source.HasField("cover") else None

        result = self._router.dispatch_prepare(domain, event_any, root)
        return result if result is not None else []

    def dispatch(
        self,
        source: types.EventBook,
        destinations: list[types.EventBook] | None = None,
    ) -> saga_pb2.SagaResponse:
        """Handle source events and produce commands.

        Returns:
            SagaResponse containing commands and events.
        """
        if not source.pages:
            return saga_pb2.SagaResponse()

        domain = source.cover.domain if source.HasField("cover") else ""
        event_page = source.pages[-1]
        event_any = self._get_event_from_page(event_page)
        if event_any is None:
            return saga_pb2.SagaResponse()

        # Check if this is a Notification (rejection)
        if event_any.type_url.endswith("Notification"):
            commands = self.dispatch_rejection(source, destinations or [])
            return saga_pb2.SagaResponse(commands=commands)

        root = source.cover.root if source.HasField("cover") else None
        correlation_id = source.cover.correlation_id if source.HasField("cover") else ""

        result = self._router.dispatch(
            domain, event_any, root, correlation_id, destinations or []
        )
        commands = result if result is not None else []
        return saga_pb2.SagaResponse(commands=commands)

    def _get_event_from_page(self, page: types.EventPage) -> any_pb2.Any | None:
        """Extract event from EventPage, handling both old and new proto formats."""
        # Try old format: direct event field
        if page.HasField("event"):
            return page.event
        # Try new format: event in payload oneof
        if hasattr(page, "payload") and page.WhichOneof("payload") == "event":
            return page.event
        # Try getter method
        if hasattr(page, "GetEvent"):
            evt = page.GetEvent()
            if evt and evt.type_url:
                return evt
        return None

    def dispatch_rejection(
        self, source: types.EventBook, destinations: list[types.EventBook]
    ) -> list[types.CommandBook]:
        """Dispatch rejection notification to handler.

        Args:
            source: The source EventBook containing the Notification.
            destinations: EventBooks for destinations (for compensating commands).

        Returns:
            List of CommandBooks (compensating commands), or empty list if no handler.
        """
        if not source.pages:
            return []

        event_page = source.pages[-1]
        if not event_page.HasField("event"):
            return []

        event_any = event_page.event
        if not event_any.type_url.endswith("Notification"):
            return []

        notification = types.Notification()
        event_any.Unpack(notification)

        root = source.cover.root if source.HasField("cover") else None
        correlation_id = source.cover.correlation_id if source.HasField("cover") else ""

        result = self._router.dispatch_rejection(
            notification, root, correlation_id, destinations
        )
        return result if result is not None else []

    next_sequence = staticmethod(Router.next_sequence)
    pack_any = staticmethod(Router.pack_any)


# ============================================================================
# Protocol-based Routers (CommandHandlerRouter, etc.)
# ============================================================================


class CommandHandlerRouter(Generic[S]):
    """Router for command handler components (commands -> events, single domain).

    Wraps a CommandHandlerDomainHandler and provides command dispatch with
    automatic state reconstruction and type-URL routing.

    Domain is set at construction time - no .domain() method exists,
    enforcing single-domain constraint.

    Example:
        class PlayerHandler(CommandHandlerDomainHandler[PlayerState]):
            def command_types(self) -> list[str]:
                return ["RegisterPlayer", "DepositFunds"]

            def state_router(self) -> StateRouter[PlayerState]:
                return self._state_router

            def handle(self, cmd_book, payload, state, seq) -> EventBook:
                # Dispatch by type_url...

        router = CommandHandlerRouter("player", "player", PlayerHandler())
        response = router.dispatch(contextual_command)
    """

    def __init__(
        self,
        name: str,
        domain: str,
        handler: CommandHandlerDomainHandler[S],
    ) -> None:
        """Create a new command handler router.

        Args:
            name: Router name (typically same as domain for aggregates).
            domain: The domain this aggregate handles.
            handler: The handler implementing AggregateDomainHandler protocol.
        """
        self._name = name
        self._domain = domain
        self._handler = handler

    @property
    def name(self) -> str:
        """Get the router name."""
        return self._name

    @property
    def domain(self) -> str:
        """Get the domain."""
        return self._domain

    def command_types(self) -> list[str]:
        """Get command types from the handler."""
        return self._handler.command_types()

    def subscriptions(self) -> list[tuple[str, list[str]]]:
        """Get subscriptions for this aggregate.

        Returns:
            List of (domain, command_types) tuples.
        """
        return [(self._domain, self.command_types())]

    def rebuild_state(self, events: types.EventBook | None) -> S:
        """Rebuild state from events using the handler's state router."""
        return self._handler.state_router().with_event_book(events)

    def dispatch(self, cmd: types.ContextualCommand) -> ch_pb2.BusinessResponse:
        """Dispatch a ContextualCommand to the handler.

        Extracts command + prior events, rebuilds state, matches type_url,
        and calls the handler. Detects Notification and routes to rejection handlers.

        Args:
            cmd: The contextual command containing command book and prior events.

        Returns:
            BusinessResponse wrapping the handler's EventBook or RevocationResponse.

        Raises:
            ValueError: If no command pages.
            CommandRejectedError: If handler rejects the command.
        """
        command_book = cmd.command
        prior_events = cmd.events if cmd.HasField("events") else None

        state = self.rebuild_state(prior_events)
        seq = _next_sequence(prior_events)

        if not command_book.pages:
            raise ValueError("No command pages")

        command_any = command_book.pages[0].command
        if not command_any.type_url:
            raise ValueError("No command pages")

        type_url = command_any.type_url

        # Check for Notification (rejection/compensation)
        if type_url == NOTIFICATION_TYPE_URL:
            notification = types.Notification()
            command_any.Unpack(notification)
            return self._dispatch_rejection(notification, state)

        # Validate command type
        type_suffix = type_url.rsplit(".", 1)[-1] if type_url else ""
        command_types = self._handler.command_types()
        if type_suffix not in command_types:
            raise ValueError(ERRMSG_UNKNOWN_COMMAND.format(type_suffix))

        # Execute handler
        events = self._handler.handle(command_book, command_any, state, seq)
        return ch_pb2.BusinessResponse(events=events)

    def _dispatch_rejection(
        self,
        notification: types.Notification,
        state: S,
    ) -> ch_pb2.BusinessResponse:
        """Dispatch a rejection Notification to the handler's on_rejected."""
        # Unpack rejection details from notification payload
        rejection = types.RejectionNotification()
        if notification.HasField("payload"):
            notification.payload.Unpack(rejection)

        domain, command_type_name = _extract_rejection_key(rejection)

        response = self._handler.on_rejected(
            notification, state, domain, command_type_name
        )

        if response.events is not None:
            return ch_pb2.BusinessResponse(events=response.events)
        elif response.notification is not None:
            return ch_pb2.BusinessResponse(notification=response.notification)
        else:
            return ch_pb2.BusinessResponse(
                revocation=ch_pb2.RevocationResponse(
                    emit_system_revocation=True,
                    reason=f"Handler returned empty response for {domain}/{command_type_name}",
                )
            )


class ProcessManagerRouter(Generic[S]):
    """Router for process manager components (events -> commands + PM events, multi-domain).

    Process managers correlate events across multiple domains and maintain
    their own state. Domains are registered via constructor or fluent .domain() calls.

    Example:
        class OrderPmHandler(ProcessManagerDomainHandler[WorkflowState]):
            def event_types(self) -> list[str]:
                return ["OrderCreated"]

            def state_router(self) -> StateRouter[WorkflowState]:
                return self._state_router

            def handle(self, trigger, event, state):
                return pm.ProcessManagerHandleResponse(commands=[new_command_book(...)])

        # Single-domain PM (simple constructor)
        router = ProcessManagerRouter("pmg-order-flow", "order", handler)

        # Multi-domain PM (with fluent .domain())
        router = (ProcessManagerRouter("pmg-order-flow", "order-flow", rebuild_state)
            .domain("order", OrderPmHandler())
            .domain("inventory", InventoryPmHandler()))

        response = router.dispatch(trigger, process_state)
    """

    def __init__(
        self,
        name: str,
        pm_domain: str,
        handler_or_rebuild: (
            ProcessManagerDomainHandler[S] | Callable[[types.EventBook], S]
        ),
    ) -> None:
        """Create a new process manager router.

        Two construction patterns:
        1. Single-domain: ProcessManagerRouter(name, domain, handler)
        2. Multi-domain: ProcessManagerRouter(name, pm_domain, rebuild_func).domain(...)

        Args:
            name: Router name (e.g., "pmg-order-flow").
            pm_domain: The PM's own domain for state storage (single-domain) or input domain (multi-domain).
            handler_or_rebuild: Either a ProcessManagerDomainHandler (single-domain)
                or a function to rebuild PM state from events (multi-domain).
        """
        self._name = name
        self._pm_domain = pm_domain
        self._domains: dict[str, ProcessManagerDomainHandler[S]] = {}
        self._rebuild: Callable[[types.EventBook], S] | None = None

        # Check if it's a handler (has event_types method) or a rebuild function
        if hasattr(handler_or_rebuild, "event_types"):
            # Single-domain mode: handler passed directly
            handler = handler_or_rebuild
            self._domains[pm_domain] = handler
            # Use handler's state_router for state reconstruction
            self._rebuild = lambda events: handler.state_router().with_event_book(
                events
            )
        else:
            # Multi-domain mode: rebuild function passed
            self._rebuild = handler_or_rebuild

    @property
    def name(self) -> str:
        """Get the router name."""
        return self._name

    @property
    def pm_domain(self) -> str:
        """Get the PM's own domain (for state storage)."""
        return self._pm_domain

    def domain(
        self,
        name: str,
        handler: ProcessManagerDomainHandler[S],
    ) -> ProcessManagerRouter[S]:
        """Register a domain handler.

        Process managers can have multiple input domains.

        Args:
            name: Domain name (e.g., "order", "inventory").
            handler: Handler implementing ProcessManagerDomainHandler protocol.

        Returns:
            Self for chaining.
        """
        self._domains[name] = handler
        return self

    def subscriptions(self) -> list[tuple[str, list[str]]]:
        """Get subscriptions (domain + event types) for this PM.

        Returns:
            List of (domain, event_types) tuples.
        """
        return [
            (domain, handler.event_types()) for domain, handler in self._domains.items()
        ]

    def rebuild_state(self, events: types.EventBook) -> S:
        """Rebuild PM state from events."""
        return self._rebuild(events)

    def prepare_destinations(
        self,
        trigger: types.EventBook | None,
        process_state: types.EventBook | None,
    ) -> list[types.Cover]:
        """Get destinations needed for the given trigger and process state.

        Args:
            trigger: Trigger EventBook with source event.
            process_state: Current PM state EventBook.

        Returns:
            List of Covers identifying destination aggregates to fetch.
        """
        if trigger is None or not trigger.pages:
            return []

        trigger_domain = trigger.cover.domain if trigger.HasField("cover") else ""

        event_page = trigger.pages[-1]
        if not event_page.HasField("event"):
            return []

        # Rebuild state from process_state if available
        if process_state is not None:
            state = self.rebuild_state(process_state)
        else:
            # Get handler to determine state type and create default
            handler = self._domains.get(trigger_domain)
            if handler is None:
                return []
            # Try to create default state - handlers should work with None-ish state
            # For now, rebuild from empty EventBook
            state = self._rebuild(types.EventBook())

        handler = self._domains.get(trigger_domain)
        if handler is None:
            return []

        return handler.prepare(trigger, state, event_page.event)

    def dispatch(
        self,
        trigger: types.EventBook,
        process_state: types.EventBook,
        destinations: list[types.EventBook] | None = None,
    ) -> pm.ProcessManagerHandleResponse:
        """Dispatch a trigger event to the appropriate handler.

        Args:
            trigger: Trigger EventBook with source event.
            process_state: Current PM state EventBook.
            destinations: EventBooks for destinations declared in prepare() (optional).

        Returns:
            HandleResponse with commands and process events.

        Raises:
            ValueError: If trigger has no events or no handler for domain.
        """
        trigger_domain = trigger.cover.domain if trigger.HasField("cover") else ""

        handler = self._domains.get(trigger_domain)
        if handler is None:
            raise ValueError(f"No handler for domain: {trigger_domain}")

        if not trigger.pages:
            raise ValueError("Trigger event book has no events")

        event_page = trigger.pages[-1]
        if not event_page.HasField("event"):
            raise ValueError("Missing event payload")

        event_any = event_page.event
        state = self.rebuild_state(process_state)

        # Check for Notification
        if event_any.type_url.endswith("Notification"):
            return self._dispatch_notification(handler, event_any, state)

        response = handler.handle(trigger, state, event_any, destinations or [])

        return pm.ProcessManagerHandleResponse(
            process_events=response.events,
            commands=response.commands,
            facts=response.facts if hasattr(response, "facts") else [],
        )

    def _dispatch_notification(
        self,
        handler: ProcessManagerDomainHandler[S],
        event_any: any_pb2.Any,
        state: S,
    ) -> pm.ProcessManagerHandleResponse:
        """Dispatch a Notification to the PM's rejection handler."""
        notification = types.Notification()
        event_any.Unpack(notification)

        rejection = types.RejectionNotification()
        if notification.HasField("payload"):
            notification.payload.Unpack(rejection)

        domain, cmd_suffix = _extract_rejection_key(rejection)

        # Call handler's on_rejected if it has one
        if hasattr(handler, "on_rejected"):
            response = handler.on_rejected(notification, state, domain, cmd_suffix)
            return pm.ProcessManagerHandleResponse(
                commands=[],
                process_events=response.events,
            )

        return pm.ProcessManagerHandleResponse()


# ============================================================================
# Backward Compatibility Aliases
# ============================================================================

# UpcasterRouter is an alias for SingleFluentRouter
UpcasterRouter = SingleFluentRouter


# ============================================================================
# Protocol-based Routers (wrap handler protocol objects)
# ============================================================================
#
# These routers wrap handler objects that implement Protocol interfaces.
# They're different from the fluent and OO routers above - they delegate
# to protocol implementations rather than using decorators.
# ============================================================================


class ProjectorRouter:
    """Router for projector components (events -> external output, multi-domain).

    Projectors consume events from one or more domains and produce external output.
    Uses protocol-based handlers (ProjectorDomainHandler).

    Design: This is a multi-domain router using protocol-based handlers,
    not the functional-handler pattern of FluentRouter.

    Example (single-domain):
        class PlayerProjectorHandler(ProjectorDomainHandler):
            def event_types(self) -> list[str]:
                return ["PlayerRegistered", "FundsDeposited"]

            def project(self, events) -> Projection:
                # Update read model
                return Projection()

        router = ProjectorRouter("prj-player", "player", PlayerProjectorHandler())
        response = router.dispatch(events)

    Example (multi-domain):
        router = (ProjectorRouter("prj-output")
            .domain("player", PlayerProjectorHandler())
            .domain("hand", HandProjectorHandler()))

        response = router.dispatch(events)
    """

    def __init__(
        self,
        name: str,
        input_domain: str | None = None,
        handler: ProjectorDomainHandler | None = None,
    ) -> None:
        """Create a new projector router.

        Two construction patterns:
        1. Single-domain: ProjectorRouter(name, domain, handler)
        2. Multi-domain: ProjectorRouter(name).domain(...).domain(...)

        Args:
            name: Router name (e.g., "prj-output").
            input_domain: Input domain (single-domain mode only).
            handler: Handler implementing ProjectorDomainHandler (single-domain mode only).
        """
        self._name = name
        self._domains: dict[str, ProjectorDomainHandler] = {}

        if input_domain is not None and handler is not None:
            self._domains[input_domain] = handler

    @property
    def name(self) -> str:
        """Get the router name."""
        return self._name

    def domain(
        self,
        name: str,
        handler: ProjectorDomainHandler,
    ) -> ProjectorRouter:
        """Register a domain handler.

        Projectors can have multiple input domains.

        Args:
            name: Domain name (e.g., "player", "hand").
            handler: Handler implementing ProjectorDomainHandler protocol.

        Returns:
            Self for chaining.
        """
        self._domains[name] = handler
        return self

    def subscriptions(self) -> list[tuple[str, list[str]]]:
        """Get subscriptions (domain + event types) for this projector.

        Returns:
            List of (domain, event_types) tuples.
        """
        return [
            (domain, handler.event_types()) for domain, handler in self._domains.items()
        ]

    def dispatch(self, events: types.EventBook) -> types.Projection:
        """Dispatch events to the appropriate handler.

        Args:
            events: EventBook containing events to project.

        Returns:
            Projection result.

        Raises:
            ValueError: If no handler for domain.
        """
        domain = events.cover.domain if events.HasField("cover") else ""

        handler = self._domains.get(domain)
        if handler is None:
            raise ValueError(f"No handler for domain: {domain}")

        return handler.project(events)
