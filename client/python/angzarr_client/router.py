"""DRY dispatch via router types.

CommandRouter replaces manual if/elif chains in aggregate handlers.
EventRouter replaces manual if/elif chains in saga event handlers.
Both auto-derive descriptors from their .on() registrations.

The @command_handler decorator simplifies handler functions by:
- Auto-unpacking commands from Any to concrete proto types
- Auto-packing returned events into EventBook
"""

from __future__ import annotations

import inspect
import typing
from functools import wraps
from typing import Any, Callable, Generic, TypeVar

from google.protobuf import any_pb2

from .proto.angzarr import aggregate_pb2 as aggregate
from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import types_pb2 as types

S = TypeVar("S")

# Error message constants.
ERRMSG_UNKNOWN_COMMAND = "Unknown command type"
ERRMSG_NO_COMMAND_PAGES = "No command pages"


# ============================================================================
# @command_handler decorator for function-based handlers
# ============================================================================


def validate_command_handler(
    func: Callable,
    command_type: type,
    cmd_param_index: int,
    decorator_name: str,
) -> str:
    """Validate a command handler's signature.

    Shared validation logic for @handles and @command_handler decorators.

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


def command_handler(command_type: type):
    """Decorator for function-based command handlers.

    Simplifies handler functions by:
    - Auto-unpacking the command from Any to the concrete proto type
    - Auto-packing returned event(s) into EventBook

    The decorated function receives the unpacked command instead of Any,
    and can return a single event or tuple of events instead of EventBook.

    Original signature (manual):
        handler(cb: CommandBook, cmd_any: Any, state: S, seq: int) -> EventBook

    Decorated signature (simplified):
        handler(cmd: ConcreteCommand, state: S, seq: int) -> Event | tuple[Event, ...]

    Example:
        @command_handler(cart_pb2.CreateCart)
        def handle_create(cmd: cart_pb2.CreateCart, state: CartState, seq: int):
            return cart_pb2.CartCreated(cart_id=cmd.cart_id)

        # Register with router:
        router = CommandRouter("cart", rebuild).on("CreateCart", handle_create)

    Args:
        command_type: The protobuf command class to unpack to.

    Raises:
        TypeError: If type hint is missing or doesn't match command_type.
    """

    def decorator(func: Callable) -> Callable:
        validate_command_handler(func, command_type, cmd_param_index=0, decorator_name="command_handler")

        @wraps(func)
        def wrapper(
            command_book: types.CommandBook,
            command_any: any_pb2.Any,
            state,
            seq: int,
        ) -> types.EventBook:
            # Unpack command
            cmd = command_type()
            command_any.Unpack(cmd)

            # Call handler with unpacked command
            result = func(cmd, state, seq)

            # Pack result into EventBook
            return _pack_events(result)

        # Preserve command type for introspection
        wrapper._command_type = command_type
        return wrapper

    return decorator


def _pack_events(result) -> types.EventBook:
    """Pack event(s) into an EventBook.

    Args:
        result: Single event, tuple of events, or None.

    Returns:
        EventBook containing packed events.
    """
    pages = []

    if result is None:
        pass
    elif isinstance(result, tuple):
        for event in result:
            pages.append(types.EventPage(event=_pack_any(event)))
    else:
        pages.append(types.EventPage(event=_pack_any(result)))

    return types.EventBook(pages=pages)


def _pack_any(event) -> any_pb2.Any:
    """Pack a protobuf message into Any."""
    event_any = any_pb2.Any()
    event_any.Pack(event, type_url_prefix="type.googleapis.com/")
    return event_any


# ============================================================================
# @reacts_to decorator for sagas and process managers
# ============================================================================


def reacts_to(event_type: type, *, input_domain: str = None, output_domain: str = None):
    """Decorator for event handler methods on Saga or ProcessManager.

    Registers the method as a handler for the given event type.
    Validates that event_type matches the method's type hint.

    The decorated method should return either:
    - A single protobuf command message
    - A tuple of protobuf command messages
    - None (no command to emit)

    Args:
        event_type: The protobuf event class to handle.
        input_domain: Source domain for this event (ProcessManager only).
        output_domain: Target domain for commands (optional override).

    Example (Saga):
        @reacts_to(OrderCompleted)
        def handle_completed(self, event: OrderCompleted) -> CreateShipment:
            return CreateShipment(order_id=event.order_id)

    Example (ProcessManager):
        @reacts_to(OrderCreated, input_domain="order")
        def on_order_created(self, event: OrderCreated) -> ReserveInventory:
            return ReserveInventory(...)

    Raises:
        TypeError: If type hint is missing or doesn't match event_type.
    """

    def decorator(method: Callable) -> Callable:
        # Validate at decoration time (event is at index 1 after self)
        validate_command_handler(
            method, event_type, cmd_param_index=1, decorator_name="reacts_to"
        )

        @wraps(method)
        def wrapper(self, *args, **kwargs):
            return method(self, *args, **kwargs)

        wrapper._is_handler = True
        wrapper._event_type = event_type
        wrapper._input_domain = input_domain
        wrapper._output_domain = output_domain
        return wrapper

    return decorator


# ============================================================================
# @prepares decorator for saga destination declaration
# ============================================================================


def prepares(event_type: type):
    """Decorator for saga prepare handler methods.

    Registers the method as a prepare handler for the given event type.
    The method is called during the Prepare phase to declare which
    destination aggregates need to be fetched before execution.

    The decorated method should return a list of Covers identifying
    the destination aggregates.

    Args:
        event_type: The protobuf event class to handle.

    Example:
        @prepares(HandStarted)
        def prepare_hand(self, event: HandStarted) -> list[Cover]:
            return [Cover(domain="hand", root=UUID(value=event.hand_root))]

    Raises:
        TypeError: If type hint is missing or doesn't match event_type.
    """

    def decorator(method: Callable) -> Callable:
        # Validate at decoration time (event is at index 1 after self)
        validate_command_handler(
            method, event_type, cmd_param_index=1, decorator_name="prepares"
        )

        @wraps(method)
        def wrapper(self, *args, **kwargs):
            return method(self, *args, **kwargs)

        wrapper._is_prepare_handler = True
        wrapper._event_type = event_type
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
# @projects decorator for projectors
# ============================================================================


def projects(event_type: type):
    """Decorator for projector event handler methods.

    Registers the method as a handler for the given event type.
    Validates that event_type matches the method's type hint.

    The decorated method should return a Projection message.

    Example:
        @projects(StockUpdated)
        def project_stock(self, event: StockUpdated) -> Projection:
            return Projection(...)

    Raises:
        TypeError: If type hint is missing or doesn't match event_type.
    """

    def decorator(method: Callable) -> Callable:
        # Validate at decoration time (event is at index 1 after self)
        validate_command_handler(
            method, event_type, cmd_param_index=1, decorator_name="projects"
        )

        @wraps(method)
        def wrapper(self, *args, **kwargs):
            return method(self, *args, **kwargs)

        wrapper._is_handler = True
        wrapper._event_type = event_type
        return wrapper

    return decorator


# ============================================================================
# @event_handler decorator for function-based event handlers
# ============================================================================


def event_handler(event_type: type):
    """Decorator for function-based event handlers (sagas, projectors).

    Simplifies handler functions by:
    - Auto-unpacking the event from Any to the concrete proto type
    - Storing event type for router reflection

    Original signature (manual):
        handler(event_any: Any, root: bytes, correlation_id: str) -> Result

    Decorated signature (simplified):
        handler(event: ConcreteEvent, root: bytes, correlation_id: str) -> Result

    Example:
        @event_handler(OrderCompleted)
        def handle_completed(event: OrderCompleted, root: bytes, corr_id: str):
            return [CommandBook(...)]

        # Register with EventRouter:
        router = EventRouter("saga", "order").on(handle_completed)

    Args:
        event_type: The protobuf event class to unpack to.

    Raises:
        TypeError: If type hint is missing or doesn't match event_type.
    """

    def decorator(func: Callable) -> Callable:
        validate_command_handler(
            func, event_type, cmd_param_index=0, decorator_name="event_handler"
        )

        @wraps(func)
        def wrapper(
            event_any: any_pb2.Any,
            root,
            correlation_id: str,
            destinations: list = None,
        ):
            # Unpack event
            event = event_type()
            event_any.Unpack(event)

            # Call handler with unpacked event
            return func(event, root, correlation_id, destinations or [])

        wrapper._event_type = event_type
        return wrapper

    return decorator


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

    The rejection handler signature:
        handler(notification: Notification, state: S) -> EventBook

    Two construction patterns:

    1. Traditional (with rebuild function)::

        router = (CommandRouter("cart", rebuild_state)
            .on("CreateCart", handle_create_cart)
            .on("AddItem", handle_add_item))

    2. Fluent (with StateRouter composition)::

        router = (CommandRouter("cart")
            .with_state(
                StateRouter(CartState)
                .on(CartCreated, apply_created)
                .on(ItemAdded, apply_item_added)
            )
            .on("CreateCart", handle_create_cart)
            .on("AddItem", handle_add_item))

    Example::

        # In Handle():
        response = router.dispatch(request)

        # For topology:
        desc = router.descriptor()
    """

    def __init__(
        self, domain: str, rebuild: Callable[[types.EventBook | None], S] = None
    ) -> None:
        self.domain = domain
        self._rebuild = rebuild
        self._state_router = None  # StateRouter for fluent composition
        self._handlers: list[tuple[str, Callable]] = []
        self._rejection_handlers: dict[str, Callable] = {}  # "domain/command" -> handler

    def with_state(self, state_router) -> "CommandRouter[S]":
        """Compose a StateRouter for state reconstruction.

        Alternative to passing rebuild function to constructor.
        The StateRouter handles event-to-state application with auto-unpacking.

        Args:
            state_router: StateRouter instance configured with event handlers.

        Returns:
            Self for chaining.

        Example::

            router = (CommandRouter("player")
                .with_state(
                    StateRouter(PlayerState)
                    .on(PlayerRegistered, apply_registered)
                    .on(FundsDeposited, apply_deposited)
                )
                .on(RegisterPlayer, handle_register))
        """
        self._state_router = state_router
        return self

    def _get_state(self, event_book: types.EventBook | None) -> S:
        """Rebuild state using configured method.

        Uses StateRouter if composed, otherwise falls back to rebuild function.
        """
        if self._state_router is not None:
            return self._state_router.with_event_book(event_book)
        elif self._rebuild is not None:
            return self._rebuild(event_book)
        else:
            raise ValueError(
                "CommandRouter requires either rebuild function in constructor "
                "or StateRouter via .with_state()"
            )

    def on(self, suffix_or_handler, handler: Callable = None) -> CommandRouter[S]:
        """Register a handler for a command type_url suffix.

        Two calling patterns:
            router.on("CreateCart", handle_create)  # Explicit suffix
            router.on(handle_create)                # Derive suffix from @command_handler

        When passing only a handler, it must be decorated with @command_handler
        so the command type can be derived via reflection.
        """
        if handler is None:
            # Single argument: derive suffix from @command_handler decorator
            handler = suffix_or_handler
            if not hasattr(handler, "_command_type"):
                raise TypeError(
                    f"{handler.__name__}: must be decorated with @command_handler "
                    "to use single-argument .on()"
                )
            suffix = handler._command_type.__name__
        else:
            suffix = suffix_or_handler

        self._handlers.append((suffix, handler))
        return self

    def on_rejected(
        self, domain: str, command: str, handler: Callable
    ) -> CommandRouter[S]:
        """Register a handler for rejected commands.

        Called when a saga/PM command targeting the specified domain and command
        type is rejected by the target aggregate.

        The handler signature:
            handler(notification: Notification, state: S) -> EventBook

        The notification.payload contains a RejectionNotification with:
        - rejected_command: The command that was rejected
        - rejection_reason: Why it was rejected
        - issuer_name: Saga/PM that issued the command
        - issuer_type: "saga" or "process_manager"
        - source_aggregate: Cover of triggering aggregate
        - source_event_sequence: Event that triggered the saga/PM

        Example:
            def handle_payment_rejected(notification, state):
                rejection = RejectionNotification()
                notification.payload.Unpack(rejection)
                return pack_events(FundsReleased(
                    amount=state.reserved_amount,
                    reason=rejection.rejection_reason,
                ))

            router.on_rejected("payment", "ProcessPayment", handle_payment_rejected)

        Args:
            domain: The target domain of the rejected command.
            command: The type name of the rejected command.
            handler: Function to handle the rejection.

        Returns:
            Self for chaining.
        """
        key = f"{domain}/{command}"
        self._rejection_handlers[key] = handler
        return self

    def dispatch(self, cmd: types.ContextualCommand) -> aggregate.BusinessResponse:
        """Dispatch a ContextualCommand to the matching handler.

        Extracts command + prior events, rebuilds state, matches type_url
        suffix, and calls the registered handler. Detects Notification
        and routes to rejection handlers.

        Returns:
            BusinessResponse wrapping the handler's EventBook or RevocationResponse.

        Raises:
            ValueError: If no command pages or no handler matches.
        """
        command_book = cmd.command
        prior_events = cmd.events if cmd.HasField("events") else None

        state = self._get_state(prior_events)
        seq = next_sequence(prior_events)

        if not command_book.pages:
            raise ValueError(ERRMSG_NO_COMMAND_PAGES)

        command_any = command_book.pages[0].command
        if not command_any.type_url:
            raise ValueError(ERRMSG_NO_COMMAND_PAGES)

        type_url = command_any.type_url

        # Check for Notification (rejection/compensation)
        if type_url.endswith("Notification"):
            notification = types.Notification()
            command_any.Unpack(notification)
            return self._dispatch_rejection(notification, state)

        # Normal command dispatch
        for suffix, handler in self._handlers:
            if type_url.endswith(suffix):
                events = handler(command_book, command_any, state, seq)
                return aggregate.BusinessResponse(events=events)

        raise ValueError(f"{ERRMSG_UNKNOWN_COMMAND}: {type_url}")

    def _dispatch_rejection(
        self, notification: types.Notification, state: S
    ) -> aggregate.BusinessResponse:
        """Dispatch a rejection Notification to the matching handler.

        Args:
            notification: The notification containing RejectionNotification payload.
            state: Current aggregate state.

        Returns:
            BusinessResponse with events or RevocationResponse.
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
                cmd_type_url = rejected_cmd.pages[0].command.type_url
                command_suffix = cmd_type_url.rsplit("/", 1)[-1] if "/" in cmd_type_url else cmd_type_url

        # Dispatch to rejection handler if found (use suffix matching like regular dispatch)
        for key, handler in self._rejection_handlers.items():
            expected_domain, expected_command = key.split("/", 1)
            if domain == expected_domain and command_suffix.endswith(expected_command):
                events = handler(notification, state)
                return aggregate.BusinessResponse(events=events)

        # Default: delegate to framework
        return aggregate.BusinessResponse(
            revocation=aggregate.RevocationResponse(
                emit_system_revocation=True,
                reason=f"Aggregate {self.domain} has no custom compensation for {domain}/{command_suffix}",
            )
        )


# ============================================================================
# Helpers
# ============================================================================


def next_sequence(events: types.EventBook | None) -> int:
    """Compute the next event sequence number from prior events."""
    if events is None or not events.pages:
        return 0
    return len(events.pages)


# ============================================================================
# EventRouter — unified event dispatch for sagas, PMs, and projectors
# ============================================================================


class EventRouter:
    """Unified event dispatcher for sagas, process managers, and projectors.

    Uses fluent `.domain().on()` pattern to register handlers with domain context.
    Subscriptions are auto-derived from registrations.

    Two-phase protocol support:
        1. prepare_destinations(source) -> list of Covers to fetch
        2. dispatch(source, destinations) -> list of CommandBooks

    The handler signature:
        handler(event: Any, root: UUID | None, correlation_id: str, destinations: list[EventBook]) -> list[CommandBook]

    The prepare handler signature:
        prepare_handler(event: Any, root: UUID | None) -> list[Cover]

    Example (Saga - single domain)::

        router = (EventRouter("saga-table-hand")
            .domain("table")
                .on("HandStarted", handle_started))

    Example (Process Manager - multi-domain)::

        router = (EventRouter("pmg-order-flow")
            .domain("order")
                .on("OrderCreated", handle_created)
            .domain("inventory")
                .on("StockReserved", handle_reserved))

    Example (Projector - multi-domain)::

        router = (EventRouter("prj-output")
            .domain("player")
                .on("PlayerRegistered", handle_registered)
            .domain("hand")
                .on("CardsDealt", handle_dealt))

    Usage::

        # Get auto-derived subscriptions
        subs = router.subscriptions()  # [("player", ["PlayerRegistered", ...]), ...]

        # In saga Execute:
        commands = router.dispatch(source_event_book, destinations)
    """

    def __init__(self, name: str, input_domain: str | None = None) -> None:
        """Create a new EventRouter.

        Args:
            name: Component name (e.g., "saga-order-fulfillment", "pmg-hand-flow")
            input_domain: (Deprecated) Single input domain. Use .domain() instead.
        """
        self.name = name
        self._current_domain: str | None = None
        # domain -> [(suffix, handler)]
        self._handlers: dict[str, list[tuple[str, Callable]]] = {}
        # domain -> {suffix: handler}
        self._prepare_handlers: dict[str, dict[str, Callable]] = {}

        # Backwards compatibility: if input_domain provided, set it as current context
        if input_domain is not None:
            self.domain(input_domain)

    def domain(self, name: str) -> "EventRouter":
        """Set the current domain context for subsequent .on() calls.

        Args:
            name: Domain name (e.g., "player", "order", "inventory")

        Returns:
            Self for chaining.
        """
        self._current_domain = name
        if name not in self._handlers:
            self._handlers[name] = []
        if name not in self._prepare_handlers:
            self._prepare_handlers[name] = {}
        return self

    def prepare(self, suffix: str, handler: Callable) -> "EventRouter":
        """Register a prepare handler for an event type_url suffix.

        The prepare handler returns a list of Covers identifying destinations
        that should be fetched before the main handler executes.

        Must be called after .domain() to set context.
        """
        if self._current_domain is None:
            raise ValueError("Must call .domain() before .prepare()")
        self._prepare_handlers[self._current_domain][suffix] = handler
        return self

    def on(self, suffix_or_handler, handler: Callable = None) -> "EventRouter":
        """Register a handler for an event type_url suffix in current domain.

        Must be called after .domain() to set context.

        Two calling patterns:
            router.domain("order").on("OrderCompleted", handle_completed)
            router.domain("order").on(handle_completed)  # Derive suffix from @event_handler

        When passing only a handler, it must be decorated with @event_handler
        so the event type can be derived via reflection.
        """
        if self._current_domain is None:
            raise ValueError("Must call .domain() before .on()")

        if handler is None:
            # Single argument: derive suffix from @event_handler decorator
            handler = suffix_or_handler
            if not hasattr(handler, "_event_type"):
                raise TypeError(
                    f"{handler.__name__}: must be decorated with @event_handler "
                    "to use single-argument .on()"
                )
            suffix = handler._event_type.__name__
        else:
            suffix = suffix_or_handler

        self._handlers[self._current_domain].append((suffix, handler))
        return self

    def subscriptions(self) -> list[tuple[str, list[str]]]:
        """Auto-derive subscriptions from registered handlers.

        Returns:
            List of (domain, event_types) tuples.
        """
        return [
            (domain, [suffix for suffix, _ in handlers])
            for domain, handlers in self._handlers.items()
            if handlers
        ]

    def prepare_destinations(self, book: types.EventBook) -> list[types.Cover]:
        """Get destinations needed for the given source events.

        Iterates pages, matches type_url suffixes against prepare handlers,
        and collects destination Covers.
        """
        root = book.cover.root if book.HasField("cover") else None
        source_domain = book.cover.domain if book.HasField("cover") else ""

        destinations: list[types.Cover] = []
        for page in book.pages:
            if not page.HasField("event"):
                continue
            # Check prepare handlers for source domain
            if source_domain in self._prepare_handlers:
                for suffix, handler in self._prepare_handlers[source_domain].items():
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

        Routes based on source domain and event type suffix.

        Args:
            book: Source EventBook containing events to process
            destinations: Optional list of destination EventBooks for two-phase protocol
        """
        root = book.cover.root if book.HasField("cover") else None
        correlation_id = book.cover.correlation_id if book.HasField("cover") else ""
        source_domain = book.cover.domain if book.HasField("cover") else ""
        dests = destinations or []

        # Find handlers for this domain
        domain_handlers = self._handlers.get(source_domain, [])
        if not domain_handlers:
            return []

        commands: list[types.CommandBook] = []
        for page in book.pages:
            if not page.HasField("event"):
                continue
            for suffix, handler in domain_handlers:
                if page.event.type_url.endswith(suffix):
                    commands.extend(handler(page.event, root, correlation_id, dests))
                    break
        return commands

    # -------------------------------------------------------------------------
    # Backwards compatibility (deprecated)
    # -------------------------------------------------------------------------

    @property
    def input_domain(self) -> str:
        """Return first registered domain (for backwards compatibility).

        Deprecated: Use .subscriptions() instead.
        """
        domains = list(self._handlers.keys())
        return domains[0] if domains else ""



# ============================================================================
# @upcaster decorator for function-based upcasters
# ============================================================================


def upcaster(from_type: type, to_type: type):
    """Decorator for function-based upcaster handlers.

    Simplifies handler functions by:
    - Auto-unpacking the old event from Any to concrete type
    - Auto-packing the new event into Any
    - Storing types for router reflection

    Original signature (manual):
        handler(event_any: Any) -> Any

    Decorated signature (simplified):
        handler(old: OldEventType) -> NewEventType

    Example:
        @upcaster(OrderCreatedV1, OrderCreated)
        def upcast_created(old: OrderCreatedV1) -> OrderCreated:
            return OrderCreated(order_id=old.order_id, total=0)

        router = UpcasterRouter("order").on(upcast_created)

    Args:
        from_type: The old event version to transform from.
        to_type: The new event version to transform to.

    Raises:
        TypeError: If type hints don't match decorator arguments.
    """

    def decorator(func: Callable) -> Callable:
        validate_command_handler(
            func, from_type, cmd_param_index=0, decorator_name="upcaster"
        )

        @wraps(func)
        def wrapper(event_any: any_pb2.Any) -> any_pb2.Any:
            # Unpack old event
            old_event = from_type()
            event_any.Unpack(old_event)

            # Transform
            new_event = func(old_event)

            # Pack new event
            new_any = any_pb2.Any()
            new_any.Pack(new_event)
            return new_any

        wrapper._from_type = from_type
        wrapper._to_type = to_type
        return wrapper

    return decorator


# ============================================================================
# UpcasterRouter — event version transformation
# ============================================================================


class UpcasterRouter:
    """DRY event transformer for upcasters.

    Matches old event type_url suffixes and transforms to new versions.
    Events without registered transformations pass through unchanged.

    Example::

        router = (UpcasterRouter("order")
            .on(upcast_created_v1)
            .on(upcast_shipped_v1))

        # Transform events:
        new_events = router.upcast(old_events)

        # For topology:
        desc = router.descriptor()
    """

    def __init__(self, domain: str) -> None:
        self.domain = domain
        self._handlers: list[tuple[str, Callable, type]] = []  # (suffix, handler, to_type)

    def on(self, suffix_or_handler, handler: Callable = None) -> UpcasterRouter:
        """Register a handler for an old event type_url suffix.

        Two calling patterns:
            router.on("OrderCreatedV1", upcast_created)  # Explicit suffix
            router.on(upcast_created)                     # Derive suffix from @upcaster

        When passing only a handler, it must be decorated with @upcaster
        so the from_type can be derived via reflection.
        """
        if handler is None:
            # Single argument: derive suffix from @upcaster decorator
            handler = suffix_or_handler
            if not hasattr(handler, "_from_type"):
                raise TypeError(
                    f"{handler.__name__}: must be decorated with @upcaster "
                    "to use single-argument .on()"
                )
            suffix = handler._from_type.__name__
            to_type = handler._to_type
        else:
            suffix = suffix_or_handler
            to_type = getattr(handler, "_to_type", None)

        self._handlers.append((suffix, handler, to_type))
        return self

    def upcast(self, events: list[types.EventPage]) -> list[types.EventPage]:
        """Transform a list of events to current versions.

        Args:
            events: List of EventPages to transform.

        Returns:
            List of EventPages with transformed events.
        """
        result = []

        for page in events:
            if not page.HasField("event"):
                result.append(page)
                continue

            type_url = page.event.type_url
            transformed = False

            for suffix, handler, _ in self._handlers:
                if type_url.endswith(suffix):
                    new_event = handler(page.event)
                    new_page = types.EventPage(event=new_event, sequence=page.sequence)
                    new_page.created_at.CopyFrom(page.created_at)
                    result.append(new_page)
                    transformed = True
                    break

            if not transformed:
                result.append(page)

        return result
