"""Unified Router module for aggregates, sagas, process managers, and projectors.

This module provides:

1. Protocol-based Routers (wrap handler objects):
   - CommandHandlerRouter, SagaRouter, ProcessManagerRouter, ProjectorRouter
   - Wrap handler protocol implementations
   - Provide subscriptions() and dispatch() methods

2. Decorators for OO-style components:
   - @handles, @applies (for Aggregate base class - defined in aggregate.py)
   - @reacts_to, @prepares (for Saga/ProcessManager base classes)
   - @projects (for Projector base class)
   - @rejected (for rejection/compensation handlers)

Usage (Protocol-based):
    router = CommandHandlerRouter("player", "player", PlayerHandler())
    router = SagaRouter("saga-order-fulfillment", "order", OrderHandler())

Usage (OO with decorators):
    class TableHandSaga(Saga):
        @prepares(HandStarted)
        def prepare_hand(self, event): ...

        @reacts_to(HandStarted)
        def handle_started(self, event, destinations): ...
"""

from __future__ import annotations

import inspect
import typing
from functools import wraps
from typing import TYPE_CHECKING, Callable, Generic, TypeVar
from typing import Any as TypingAny

from google.protobuf import any_pb2

from .errors import CommandRejectedError
from .handler_protocols import (
    CommandHandlerDomainHandler,
    ProcessManagerDomainHandler,
    ProcessManagerResponse,
    ProjectorDomainHandler,
    SagaDomainHandler,
)
from .helpers import TYPE_URL_PREFIX
from .proto.angzarr import command_handler_pb2 as command_handler
from .proto.angzarr import process_manager_pb2 as pm
from .proto.angzarr import projector_pb2 as projector
from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import types_pb2 as types

if TYPE_CHECKING:
    from .state_builder import StateRouter

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
                types.EventPage(sequence=start_seq + i, event=_pack_any(event))
            )
    else:
        pages.append(types.EventPage(sequence=start_seq, event=_pack_any(result)))

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
# Protocol-based Routers (CommandHandlerRouter, SagaRouter, etc.)
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

    def dispatch(
        self, cmd: types.ContextualCommand
    ) -> command_handler.BusinessResponse:
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
        return command_handler.BusinessResponse(events=events)

    def _dispatch_rejection(
        self,
        notification: types.Notification,
        state: S,
    ) -> command_handler.BusinessResponse:
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
            return command_handler.BusinessResponse(events=response.events)
        elif response.notification is not None:
            return command_handler.BusinessResponse(notification=response.notification)
        else:
            return command_handler.BusinessResponse(
                revocation=command_handler.RevocationResponse(
                    emit_system_revocation=True,
                    reason=f"Handler returned empty response for {domain}/{command_type_name}",
                )
            )


class SagaRouter:
    """Router for saga components (events -> commands, single domain, stateless).

    Wraps a SagaDomainHandler and provides event dispatch with two-phase
    protocol support (prepare_destinations + dispatch).

    Domain is set at construction time - no .domain() method exists,
    enforcing single-domain constraint.

    Example:
        class OrderSagaHandler(SagaDomainHandler):
            def event_types(self) -> list[str]:
                return ["OrderCompleted"]

            def prepare(self, source, event) -> list[Cover]:
                return [Cover(domain="fulfillment", root=...)]

            def execute(self, source, event, destinations) -> list[CommandBook]:
                return [new_command_book(...)]

        router = SagaRouter("saga-order-fulfillment", "order", OrderSagaHandler())
        destinations = router.prepare_destinations(source_events)
        response = router.dispatch(source_events, destinations)
    """

    def __init__(
        self,
        name: str,
        input_domain: str,
        handler: SagaDomainHandler,
    ) -> None:
        """Create a new saga router.

        Args:
            name: Router name (e.g., "saga-order-fulfillment").
            input_domain: The domain this saga subscribes to.
            handler: The handler implementing SagaDomainHandler protocol.
        """
        self._name = name
        self._input_domain = input_domain
        self._handler = handler

    @property
    def name(self) -> str:
        """Get the router name."""
        return self._name

    @property
    def input_domain(self) -> str:
        """Get the input domain."""
        return self._input_domain

    def event_types(self) -> list[str]:
        """Get event types from the handler."""
        return self._handler.event_types()

    def subscriptions(self) -> list[tuple[str, list[str]]]:
        """Get subscriptions for this saga.

        Returns:
            List of (domain, event_types) tuples.
        """
        return [(self._input_domain, self.event_types())]

    def prepare_destinations(
        self,
        source: types.EventBook | None,
    ) -> list[types.Cover]:
        """Get destinations needed for the given source events.

        Calls the handler's prepare() method for the last event in the source.

        Args:
            source: Source EventBook containing triggering events.

        Returns:
            List of Covers identifying destination aggregates to fetch.
        """
        if source is None or not source.pages:
            return []

        event_page = source.pages[-1]
        if not event_page.HasField("event"):
            return []

        return self._handler.prepare(source, event_page.event)

    def dispatch(
        self,
        source: types.EventBook,
        destinations: list[types.EventBook],
    ) -> saga.SagaResponse:
        """Dispatch an event to the saga handler.

        Args:
            source: Source EventBook containing triggering events.
            destinations: EventBooks for destinations declared in prepare().

        Returns:
            SagaResponse containing commands to send.

        Raises:
            ValueError: If source has no events.
        """
        if not source.pages:
            raise ValueError("Source event book has no events")

        event_page = source.pages[-1]
        if not event_page.HasField("event"):
            raise ValueError("Missing event payload")

        # Check if event type matches handler's event_types
        event_any = event_page.event
        type_suffix = (
            event_any.type_url.rsplit(".", 1)[-1] if event_any.type_url else ""
        )
        event_types = self._handler.event_types()
        if type_suffix not in event_types:
            # Event type doesn't match - return empty commands
            return saga.SagaResponse(commands=[])

        commands = self._handler.execute(source, event_any, destinations)

        return saga.SagaResponse(commands=commands)


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
    ) -> "ProcessManagerRouter[S]":
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
            commands=response.commands,
            process_events=response.events,
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


class ProjectorRouter:
    """Router for projector components (events -> external output, single or multi-domain).

    Projectors consume events from one or more domains and produce external output.
    Single-domain projectors use the simple constructor; multi-domain projectors
    register domains via fluent .domain() calls.

    Example (single-domain):
        class PlayerProjectorHandler(ProjectorDomainHandler):
            def event_types(self) -> list[str]:
                return ["PlayerRegistered", "FundsDeposited"]

            def project(self, source, event) -> ProjectorResponse:
                # Update read model
                return ProjectorResponse(projections=[...])

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
    ) -> "ProjectorRouter":
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
