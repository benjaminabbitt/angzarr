"""Unified Router pattern for aggregates, sagas, process managers, and projectors.

These routers wrap handler protocol implementations and provide:
- subscriptions() for auto-deriving what types/domains are handled
- dispatch() for routing to handlers by type_url matching
- prepare_destinations() (for sagas/PMs) for two-phase protocol support

Two router categories based on domain cardinality:
- Single-domain: AggregateRouter, SagaRouter (domain set at construction)
- Multi-domain: ProcessManagerRouter, ProjectorRouter (fluent .domain() method)

Usage:
    # Aggregate (single domain)
    router = AggregateRouter("player", "player", PlayerHandler())

    # Saga (single domain)
    router = SagaRouter("saga-order-fulfillment", "order", OrderHandler())

    # Process Manager (multi-domain)
    router = (ProcessManagerRouter("pmg-hand-flow", "hand-flow", rebuild_state)
        .domain("order", OrderPmHandler())
        .domain("inventory", InventoryPmHandler()))

    # Projector (multi-domain)
    router = (ProjectorRouter("prj-output")
        .domain("player", PlayerProjectorHandler())
        .domain("hand", HandProjectorHandler()))
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Callable, Generic, TypeVar

from google.protobuf.any_pb2 import Any

from .errors import CommandRejectedError
from .handler_protocols import (
    AggregateDomainHandler,
    ProcessManagerDomainHandler,
    ProcessManagerResponse,
    ProjectorDomainHandler,
    SagaDomainHandler,
)
from .helpers import TYPE_URL_PREFIX
from .proto.angzarr import aggregate_pb2 as aggregate
from .proto.angzarr import process_manager_pb2 as pm
from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import types_pb2 as types

if TYPE_CHECKING:
    from .state_builder import StateRouter

S = TypeVar("S")

# Type URL for Notification messages
NOTIFICATION_TYPE_URL = TYPE_URL_PREFIX + "angzarr.Notification"


def _next_sequence(events: types.EventBook | None) -> int:
    """Compute the next event sequence number from prior events."""
    if events is None or not events.pages:
        return 0
    return len(events.pages)


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
# AggregateRouter - Single Domain
# ============================================================================


class AggregateRouter(Generic[S]):
    """Router for aggregate components (commands -> events, single domain).

    Wraps an AggregateDomainHandler and provides command dispatch with
    automatic state reconstruction and type-URL routing.

    Domain is set at construction time - no .domain() method exists,
    enforcing single-domain constraint.

    Example:
        class PlayerHandler(AggregateDomainHandler[PlayerState]):
            def command_types(self) -> list[str]:
                return ["RegisterPlayer", "DepositFunds"]

            def state_router(self) -> StateRouter[PlayerState]:
                return self._state_router

            def handle(self, cmd_book, payload, state, seq) -> EventBook:
                # Dispatch by type_url...

        router = AggregateRouter("player", "player", PlayerHandler())
        response = router.dispatch(contextual_command)
    """

    def __init__(
        self,
        name: str,
        domain: str,
        handler: AggregateDomainHandler[S],
    ) -> None:
        """Create a new aggregate router.

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

    def dispatch(self, cmd: types.ContextualCommand) -> aggregate.BusinessResponse:
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

        # Execute handler
        events = self._handler.handle(command_book, command_any, state, seq)
        return aggregate.BusinessResponse(events=events)

    def _dispatch_rejection(
        self,
        notification: types.Notification,
        state: S,
    ) -> aggregate.BusinessResponse:
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
            return aggregate.BusinessResponse(events=response.events)
        elif response.notification is not None:
            return aggregate.BusinessResponse(notification=response.notification)
        else:
            return aggregate.BusinessResponse(
                revocation=aggregate.RevocationResponse(
                    emit_system_revocation=True,
                    reason=f"Handler returned empty response for {domain}/{command_type_name}",
                )
            )


# ============================================================================
# SagaRouter - Single Domain
# ============================================================================


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

        commands = self._handler.execute(source, event_page.event, destinations)

        return saga.SagaResponse(commands=commands)


# ============================================================================
# ProcessManagerRouter - Multi Domain
# ============================================================================


class ProcessManagerRouter(Generic[S]):
    """Router for process manager components (events -> commands + PM events, multi-domain).

    Process managers correlate events across multiple domains and maintain
    their own state. Domains are registered via fluent .domain() calls.

    Example:
        class OrderPmHandler(ProcessManagerDomainHandler[WorkflowState]):
            def event_types(self) -> list[str]:
                return ["OrderCreated"]

            def prepare(self, trigger, state, event) -> list[Cover]:
                return []

            def handle(self, trigger, state, event, destinations):
                return ProcessManagerResponse(
                    commands=[new_command_book(...)]
                )

        router = (ProcessManagerRouter("pmg-order-flow", "order-flow", rebuild_state)
            .domain("order", OrderPmHandler())
            .domain("inventory", InventoryPmHandler()))

        response = router.dispatch(trigger, process_state, destinations)
    """

    def __init__(
        self,
        name: str,
        pm_domain: str,
        rebuild: Callable[[types.EventBook], S],
    ) -> None:
        """Create a new process manager router.

        Args:
            name: Router name (e.g., "pmg-order-flow").
            pm_domain: The PM's own domain for state storage.
            rebuild: Function to rebuild PM state from events.
        """
        self._name = name
        self._pm_domain = pm_domain
        self._rebuild = rebuild
        self._domains: dict[str, ProcessManagerDomainHandler[S]] = {}

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
        from dataclasses import fields, is_dataclass

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
        destinations: list[types.EventBook],
    ) -> pm.ProcessManagerHandleResponse:
        """Dispatch a trigger event to the appropriate handler.

        Args:
            trigger: Trigger EventBook with source event.
            process_state: Current PM state EventBook.
            destinations: EventBooks for destinations declared in prepare().

        Returns:
            ProcessManagerHandleResponse with commands and process events.

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

        response = handler.handle(trigger, state, event_any, destinations)

        return pm.ProcessManagerHandleResponse(
            commands=response.commands,
            process_events=response.events,
        )

    def _dispatch_notification(
        self,
        handler: ProcessManagerDomainHandler[S],
        event_any: Any,
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
# ProjectorRouter - Multi Domain
# ============================================================================


class ProjectorRouter:
    """Router for projector components (events -> external output, multi-domain).

    Projectors consume events from multiple domains and produce external output.
    Domains are registered via fluent .domain() calls.

    Example:
        class PlayerProjectorHandler(ProjectorDomainHandler):
            def event_types(self) -> list[str]:
                return ["PlayerRegistered", "FundsDeposited"]

            def project(self, events) -> Projection:
                # Update read model
                return Projection()

        router = (ProjectorRouter("prj-output")
            .domain("player", PlayerProjectorHandler())
            .domain("hand", HandProjectorHandler()))

        projection = router.dispatch(events)
    """

    def __init__(self, name: str) -> None:
        """Create a new projector router.

        Args:
            name: Router name (e.g., "prj-output").
        """
        self._name = name
        self._domains: dict[str, ProjectorDomainHandler] = {}

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
