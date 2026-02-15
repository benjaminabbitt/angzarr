"""Base aggregate class and @handles decorator for rich domain models.

This module provides the framework for implementing event-sourced aggregates
using the rich domain model pattern. Business logic lives as methods on the
aggregate class, with the @handles decorator registering command handlers
and capturing emitted events.

Example usage:
    from angzarr_client import Aggregate, handles

    class Player(Aggregate[_PlayerState]):
        domain = "player"

        @handles(RegisterPlayer)
        def register(self, cmd: RegisterPlayer) -> PlayerRegistered:
            if self.exists:
                raise CommandRejectedError("Player already exists")
            return PlayerRegistered(name=cmd.name, ...)

        def _create_empty_state(self) -> _PlayerState:
            return _PlayerState()

        def _apply_event(self, state, event_any):
            # Apply event to state
            ...
"""

from __future__ import annotations

import inspect
from abc import ABC, abstractmethod
from functools import wraps
from typing import TypeVar, Generic, Callable, Any as TypingAny

from google.protobuf.any_pb2 import Any

from .proto.angzarr import aggregate_pb2 as aggregate
from .proto.angzarr import saga_pb2 as saga
from .proto.angzarr import types_pb2 as types
from .router import COMPONENT_AGGREGATE, Descriptor, TargetDesc, validate_command_handler


def handles(command_type: type):
    """Decorator for command handler methods on Aggregate subclasses.

    Registers the method as a handler for the given command type.
    Validates that command_type matches the method's type hint.
    Captures returned event(s), applies to state, and records in event book.

    The decorated method should return either:
    - A single protobuf event message
    - A tuple of protobuf event messages (for multi-event operations)

    Example:
        @handles(RegisterPlayer)
        def register(self, cmd: RegisterPlayer) -> PlayerRegistered:
            # Validation and business logic...
            return PlayerRegistered(name=cmd.name, ...)

    Raises:
        TypeError: If type hint is missing or doesn't match command_type.
    """

    def decorator(method: Callable) -> Callable:
        # Validate at decoration time (cmd is at index 1 after self)
        validate_command_handler(method, command_type, cmd_param_index=1, decorator_name="handles")

        @wraps(method)
        def wrapper(self, *args, **kwargs):
            result = method(self, *args, **kwargs)
            if isinstance(result, tuple):
                for event in result:
                    self._apply_and_record(event)
            else:
                self._apply_and_record(result)
            return result

        wrapper._is_handler = True
        wrapper._command_type = command_type
        return wrapper

    return decorator


StateT = TypeVar("StateT")


class Aggregate(Generic[StateT], ABC):
    """Base class for event-sourced aggregates.

    Provides:
    - Command dispatch via @handles decorated methods
    - Rejection dispatch via @rejected decorated methods
    - Event book management (storage and retrieval)
    - State caching with lazy rebuild
    - Event recording via _apply_and_record()
    - Clearing consumed events on rebuild

    Subclasses must:
    - Set `domain` class attribute
    - Implement `_create_empty_state() -> StateT`
    - Implement `_apply_event(state: StateT, event_any: Any) -> None`
    - Decorate command handlers with `@handles(CommandType)`
    - Optionally decorate rejection handlers with `@rejected(domain, command)`

    Usage:
        class Player(Aggregate[_PlayerState]):
            domain = "player"

            def _create_empty_state(self) -> _PlayerState:
                return _PlayerState()

            def _apply_event(self, state: _PlayerState, event_any: Any) -> None:
                type_url = event_any.type_url
                if type_url.endswith("PlayerRegistered"):
                    event = PlayerRegistered()
                    event_any.Unpack(event)
                    state.player_id = event.player_id
                    # ... apply fields

            @handles(RegisterPlayer)
            def register(self, cmd: RegisterPlayer) -> PlayerRegistered:
                # Business logic...
                return PlayerRegistered(...)

            @rejected(domain="payment", command="ProcessPayment")
            def handle_payment_rejected(self, revoke_cmd) -> FundsReleased:
                return FundsReleased(amount=self.state.reserved_amount)
    """

    domain: str
    _dispatch_table: dict[str, tuple[str, type]] = {}
    _rejection_table: dict[str, str] = {}  # "domain/command" -> method_name

    def __init_subclass__(cls, **kwargs):
        super().__init_subclass__(**kwargs)

        # Skip validation for abstract intermediate classes
        if inspect.isabstract(cls):
            return

        # Validate domain is set
        if not getattr(cls, "domain", None):
            raise TypeError(f"{cls.__name__} must define 'domain' class attribute")

        cls._dispatch_table = cls._build_dispatch_table()
        cls._rejection_table = cls._build_rejection_table()

    @classmethod
    def _build_dispatch_table(cls) -> dict[str, tuple[str, type]]:
        """Scan for @handles methods and build dispatch table."""
        table = {}
        for name in dir(cls):
            attr = getattr(cls, name, None)
            if callable(attr) and getattr(attr, "_is_handler", False):
                cmd_type = attr._command_type
                suffix = cmd_type.__name__
                if suffix in table:
                    raise TypeError(
                        f"{cls.__name__}: duplicate handler for {suffix}"
                    )
                table[suffix] = (name, cmd_type)
        return table

    @classmethod
    def _build_rejection_table(cls) -> dict[str, str]:
        """Scan for @rejected methods and build rejection dispatch table.

        Returns:
            Dict mapping "domain/command" keys to method names.
        """
        table = {}
        for name in dir(cls):
            attr = getattr(cls, name, None)
            if callable(attr) and getattr(attr, "_is_rejection_handler", False):
                domain = attr._rejection_domain
                command = attr._rejection_command
                key = f"{domain}/{command}"
                if key in table:
                    raise TypeError(
                        f"{cls.__name__}: duplicate rejection handler for {key}"
                    )
                table[key] = name
        return table

    def __init__(self, event_book: types.EventBook = None):
        """Initialize aggregate with optional event book for rehydration.

        Args:
            event_book: Existing event book to rebuild state from.
                       If None, starts with empty state and empty event book.
        """
        if event_book is None:
            event_book = types.EventBook()
        self._event_book = event_book
        self._state: StateT = None

    def dispatch(self, command_any: Any) -> None:
        """Dispatch command to matching @handles method.

        Args:
            command_any: Packed command as google.protobuf.Any

        Raises:
            ValueError: If no handler matches the command type.
        """
        type_url = command_any.type_url

        for suffix, (method_name, cmd_type) in self._dispatch_table.items():
            if type_url.endswith(suffix):
                cmd = cmd_type()
                command_any.Unpack(cmd)
                getattr(self, method_name)(cmd)
                return

        raise ValueError(f"Unknown command: {type_url}")

    @classmethod
    def handle(cls, request: types.ContextualCommand) -> aggregate.BusinessResponse:
        """Handle a full gRPC request.

        Creates aggregate instance, dispatches command, returns event book.
        Detects Notification and routes to handle_revocation().

        Args:
            request: ContextualCommand with command and prior events.

        Returns:
            BusinessResponse wrapping the new events or RevocationResponse.

        Raises:
            ValueError: If no command pages in request.
        """
        prior_events = request.events if request.HasField("events") else None
        agg = cls(prior_events)

        if not request.command.pages:
            raise ValueError("No command pages")

        command_any = request.command.pages[0].command

        # Check for Notification (rejection/compensation)
        if command_any.type_url.endswith("Notification"):
            notification = types.Notification()
            command_any.Unpack(notification)
            return agg.handle_revocation(notification)

        agg.dispatch(command_any)
        return aggregate.BusinessResponse(events=agg.event_book())

    def handle_revocation(self, notification: types.Notification) -> aggregate.BusinessResponse:
        """Handle a rejection notification.

        Called when a saga/PM command is rejected and compensation is needed.
        Dispatches to @rejected decorated methods based on target domain
        and rejected command type.

        If no matching @rejected handler is found, delegates to framework.

        Args:
            notification: Notification containing RejectionNotification payload.

        Returns:
            BusinessResponse with either:
            - events: Compensation events to emit
            - revocation: RevocationResponse flags for framework action

        Usage:
            @rejected(domain="payment", command="ProcessPayment")
            def handle_payment_rejected(self, notification) -> FundsReleased:
                rejection = types.RejectionNotification()
                notification.payload.Unpack(rejection)
                return FundsReleased(amount=self.state.reserved_amount)
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
                # Extract suffix (e.g., "ProcessPayment" from "type.googleapis.com/.../ProcessPayment")
                command_suffix = type_url.rsplit("/", 1)[-1] if "/" in type_url else type_url

        # Dispatch to @rejected handler if found (use suffix matching like regular dispatch)
        for key, method_name in self._rejection_table.items():
            expected_domain, expected_command = key.split("/", 1)
            if domain == expected_domain and command_suffix.endswith(expected_command):
                # Ensure state is built before calling handler
                _ = self._get_state()
                # Call the handler (wrapper will auto-apply events)
                getattr(self, method_name)(notification)
                return aggregate.BusinessResponse(events=self.event_book())

        # Default: request framework to emit system revocation event
        return aggregate.BusinessResponse(
            revocation=aggregate.RevocationResponse(
                emit_system_revocation=True,
                reason=f"Aggregate {self.domain} has no custom compensation for {domain}/{command_suffix}",
            )
        )

    @classmethod
    def descriptor(cls) -> Descriptor:
        """Build component descriptor for topology discovery."""
        return Descriptor(
            name=cls.domain,
            component_type=COMPONENT_AGGREGATE,
            inputs=[TargetDesc(domain=cls.domain, types=list(cls._dispatch_table.keys()))],
        )

    @classmethod
    def replay(cls, request: aggregate.ReplayRequest) -> aggregate.ReplayResponse:
        """Replay events to compute state (for conflict detection).

        Creates aggregate from base snapshot, applies events, returns final state.
        Auto-implemented by base class using _create_empty_state and _apply_event.

        Args:
            request: ReplayRequest with base_snapshot and events to apply.

        Returns:
            ReplayResponse with computed state.
        """
        # Create aggregate with events from request
        event_book = types.EventBook(
            snapshot=request.base_snapshot if request.HasField("base_snapshot") else None,
            pages=list(request.events),
        )
        agg = cls(event_book)

        # Force state rebuild
        state = agg._get_state()

        # Serialize state to Any
        state_any = Any()
        state_any.Pack(state, type_url_prefix="type.googleapis.com/")

        return aggregate.ReplayResponse(state=state_any)


    def event_book(self) -> types.EventBook:
        """Return the event book for persistence.

        Contains only new events generated during this session.
        Events from rehydration are cleared after being applied.
        """
        return self._event_book

    @property
    def state(self) -> StateT:
        """Get current state (convenience property for _get_state)."""
        return self._get_state()

    @property
    def exists(self) -> bool:
        """Returns True if this aggregate has prior events (not new)."""
        # Check if we had events before rebuild cleared them, or have new events
        return self._state is not None or len(self._event_book.pages) > 0

    def _get_state(self) -> StateT:
        """Get current state, rebuilding from events if needed."""
        if self._state is None:
            self._state = self._rebuild()
        return self._state

    def _rebuild(self) -> StateT:
        """Rebuild state from event book, then clear applied events.

        The events are cleared after being applied because they've been
        "consumed" - only new events should be in the book when returned
        for persistence.
        """
        state = self._create_empty_state()
        for page in self._event_book.pages:
            if page.event:
                self._apply_event(state, page.event)
        # Clear consumed events - only new events will be in the book
        del self._event_book.pages[:]
        return state

    def _apply_and_record(self, event) -> None:
        """Pack event, apply to cached state, add to event book.

        This is called by the @handles decorator for each returned event.
        """
        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.googleapis.com/")

        # Apply directly to cached state (avoids full rebuild)
        if self._state is not None:
            self._apply_event(self._state, event_any)

        # Record in event book
        page = types.EventPage(event=event_any)
        self._event_book.pages.append(page)

    @abstractmethod
    def _create_empty_state(self) -> StateT:
        """Create an empty state instance. Must be implemented by subclasses."""
        ...

    @abstractmethod
    def _apply_event(self, state: StateT, event_any: Any) -> None:
        """Apply a single event to state. Must be implemented by subclasses."""
        ...
