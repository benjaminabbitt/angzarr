"""StateBuilder and StateRouter for declarative event handler registration.

Replaces manual if/elif chains in rebuild_state functions.

StateBuilder: Low-level, handlers receive raw Any and do their own unpacking.
StateRouter: Higher-level, auto-unpacks events to typed handlers.
"""

from __future__ import annotations

from collections.abc import Callable
from typing import Generic, TypeVar

from google.protobuf.any_pb2 import Any as AnyProto

from .helpers import TYPE_URL_PREFIX
from .proto.angzarr import types_pb2 as types

S = TypeVar("S")

# Type aliases for clarity
StateApplier = Callable[[S, AnyProto], None]
SnapshotLoader = Callable[[S, AnyProto], None]
StateFactory = Callable[[], S]


class StateBuilder(Generic[S]):
    """Builds state from events with registered handlers.

    Each handler receives the raw protobuf Any and is responsible
    for decoding. This matches CommandRouter's pattern.

    Example::

        state_builder = (StateBuilder(OrderState)
            .with_snapshot(load_order_snapshot)
            .on("OrderCreated", apply_order_created)
            .on("OrderCompleted", apply_order_completed))

        def apply_order_created(state: OrderState, event: AnyProto) -> None:
            e = order.OrderCreated()
            event.Unpack(e)
            state.customer_id = e.customer_id
            # ...

        def rebuild_state(event_book: types.EventBook | None) -> OrderState:
            return state_builder.rebuild(event_book)
    """

    def __init__(self, state_factory: StateFactory[S]) -> None:
        """Create a StateBuilder for state type S.

        Args:
            state_factory: Callable that returns a default/zero state.
        """
        self._state_factory = state_factory
        self._snapshot_loader: SnapshotLoader[S] | None = None
        self._appliers: list[tuple[str, StateApplier[S]]] = []

    def with_snapshot(self, loader: SnapshotLoader[S]) -> StateBuilder[S]:
        """Set a snapshot loader for restoring state from snapshots.

        Args:
            loader: Function that receives (state, snapshot_any) and populates state.
        """
        self._snapshot_loader = loader
        return self

    def on(self, event_type, apply: StateApplier[S]) -> StateBuilder[S]:
        """Register an event applier for an event type.

        The applier function is responsible for decoding the event.

        Args:
            event_type: Proto class (e.g., PlayerRegistered) or full type name string.
            apply: Function receiving (state, raw_any_proto).
        """
        # Extract full_name from proto class or use string directly
        if hasattr(event_type, "DESCRIPTOR"):
            full_name = event_type.DESCRIPTOR.full_name
        else:
            full_name = event_type

        self._appliers.append((full_name, apply))
        return self

    def apply(self, state: S, event: AnyProto) -> None:
        """Apply a single event to state using registered handlers.

        Useful for applying newly-created events to current state
        without going through full EventBook reconstruction.
        """
        if event is None:
            return
        for full_name, applier in self._appliers:
            if event.type_url == TYPE_URL_PREFIX + full_name:
                applier(state, event)
                break

    def rebuild(self, event_book: types.EventBook | None) -> S:
        """Rebuild state from an EventBook.

        Handles snapshots first (if loader configured), then applies events.
        Unknown event types are silently ignored.
        """
        state = self._state_factory()

        if event_book is None:
            return state

        # Load snapshot if present and loader configured
        if (
            self._snapshot_loader is not None
            and event_book.HasField("snapshot")
            and event_book.snapshot.HasField("state")
        ):
            self._snapshot_loader(state, event_book.snapshot.state)

        # Apply events
        for page in event_book.pages:
            if not page.HasField("event"):
                continue
            self.apply(state, page.event)

        return state


class StateRouter(Generic[S]):
    """Fluent state reconstruction router with auto-unpacking.

    Higher-level than StateBuilder. Handlers receive typed events,
    unpacking is automatic based on registered event types.

    Designed for composition into CommandRouter via .with_state().

    Example::

        def apply_registered(state: PlayerState, event: PlayerRegistered):
            state.player_id = f"player_{event.email}"
            state.display_name = event.display_name
            state.status = "active"

        def apply_deposited(state: PlayerState, event: FundsDeposited):
            if event.new_balance:
                state.bankroll = event.new_balance.amount

        player_state_router = (
            StateRouter(PlayerState)
            .on(PlayerRegistered, apply_registered)
            .on(FundsDeposited, apply_deposited)
        )

        # Standalone usage:
        state = player_state_router.with_events(event_book.pages)

        # Composed into CommandRouter:
        player_router = (
            CommandRouter("player")
            .with_state(player_state_router)
            .on(RegisterPlayer, handle_register)
        )
    """

    def __init__(self, state_factory: StateFactory[S]) -> None:
        """Create a StateRouter for state type S.

        Args:
            state_factory: Callable that returns a default/zero state.
                          Can be a class (e.g., PlayerState) or a factory function.
        """
        self._state_factory = state_factory
        self._handlers: list[tuple[str, type, Callable[[S, object], None]]] = []
        self._snapshot_loader: SnapshotLoader[S] | None = None

    def on(
        self, event_type: type, handler: Callable[[S, object], None]
    ) -> StateRouter[S]:
        """Register a handler for an event type.

        The handler receives typed events (auto-unpacked from Any).

        Args:
            event_type: The protobuf event class to handle.
            handler: Function(state, event) that mutates state.

        Returns:
            Self for chaining.
        """
        full_name = event_type.DESCRIPTOR.full_name
        self._handlers.append((full_name, event_type, handler))
        return self

    def with_snapshot(self, loader: SnapshotLoader[S]) -> StateRouter[S]:
        """Set a snapshot loader for restoring state from snapshots.

        Args:
            loader: Function that receives (state, snapshot_any) and populates state.

        Returns:
            Self for chaining.
        """
        self._snapshot_loader = loader
        return self

    def with_events(self, pages: list) -> S:
        """Create fresh state, apply events, return final state.

        This is the terminal operation for standalone usage.

        Args:
            pages: List of EventPage from an EventBook.

        Returns:
            Reconstructed state with all events applied.
        """
        state = self._state_factory()
        for page in pages:
            if not hasattr(page, "event") or page.event is None:
                continue
            self._apply_single(state, page.event)
        return state

    def with_event_book(self, event_book: types.EventBook | None) -> S:
        """Create fresh state from an EventBook, handling snapshots.

        Args:
            event_book: EventBook with optional snapshot and pages.

        Returns:
            Reconstructed state.
        """
        state = self._state_factory()

        if event_book is None:
            return state

        # Load snapshot if present and loader configured
        if (
            self._snapshot_loader is not None
            and event_book.HasField("snapshot")
            and event_book.snapshot.HasField("state")
        ):
            self._snapshot_loader(state, event_book.snapshot.state)

        # Apply events
        for page in event_book.pages:
            if not page.HasField("event"):
                continue
            self._apply_single(state, page.event)

        return state

    def apply_single(self, state: S, event_any: AnyProto) -> None:
        """Apply a single event to existing state.

        Useful for applying newly-created events to current state.

        Args:
            state: State to mutate.
            event_any: Packed event as Any.
        """
        self._apply_single(state, event_any)

    def _apply_single(self, state: S, event_any: AnyProto) -> None:
        """Internal: apply one event to state."""
        if event_any is None:
            return

        type_url = event_any.type_url
        for full_name, event_type, handler in self._handlers:
            if type_url == TYPE_URL_PREFIX + full_name:
                event = event_type()
                event_any.Unpack(event)
                handler(state, event)
                return

        # Unknown event type - silently ignore (forward compatibility)


class CommandRouter(Generic[S]):
    """Fluent command router for functional aggregate handlers.

    Composes with StateRouter for state reconstruction and provides
    fluent registration of command handlers.

    This is the functional alternative to the OO CommandHandler base class.
    Handlers are pure functions with signature:
        (cmd: CommandType, state: StateType, seq: int) -> Event

    Example::

        from angzarr_client import CommandRouter, StateRouter

        state_router = (
            StateRouter(PlayerState)
            .on(PlayerRegistered, apply_registered)
            .on(FundsDeposited, apply_deposited)
        )

        router = (
            CommandRouter[PlayerState]("player")
            .with_state(state_router)
            .on(RegisterPlayer, handle_register)
            .on(DepositFunds, handle_deposit)
        )

        # Use with run_command_handler_server:
        run_command_handler_server(router, "50301", logger=logger)
    """

    def __init__(self, domain: str) -> None:
        """Create a CommandRouter for a domain.

        Args:
            domain: The domain name (e.g., "player", "table").
        """
        self._domain = domain
        self._state_router: StateRouter[S] | None = None
        # command_type -> (full_name, cmd_type, handler)
        self._handlers: list[tuple[str, type, Callable]] = []

    @property
    def domain(self) -> str:
        """Get the domain name."""
        return self._domain

    def with_state(self, state_router: StateRouter[S]) -> CommandRouter[S]:
        """Compose with a StateRouter for state reconstruction.

        Args:
            state_router: StateRouter to use for rebuilding state from events.

        Returns:
            Self for chaining.
        """
        self._state_router = state_router
        return self

    def on(self, command_type: type, handler: Callable) -> CommandRouter[S]:
        """Register a command handler.

        The handler function must have signature:
            (cmd: CommandType, state: StateType, seq: int) -> Event

        Args:
            command_type: The protobuf command class to handle.
            handler: Handler function.

        Returns:
            Self for chaining.
        """
        full_name = command_type.DESCRIPTOR.full_name
        self._handlers.append((full_name, command_type, handler))
        return self

    def command_types(self) -> list[str]:
        """Get list of registered command type names."""
        return [name.rsplit(".", 1)[-1] for name, _, _ in self._handlers]

    def dispatch(
        self, request: "types.ContextualCommand"
    ) -> "command_handler.BusinessResponse":
        """Dispatch a ContextualCommand to the appropriate handler.

        Rebuilds state from prior events, unpacks command, calls handler,
        and packs result into BusinessResponse.

        Args:
            request: ContextualCommand with command and prior events.

        Returns:
            BusinessResponse wrapping the new events.

        Raises:
            ValueError: If no command pages or unknown command type.
        """
        # Import here to avoid circular imports
        from .proto.angzarr import command_handler_pb2 as command_handler

        prior_events = request.events if request.HasField("events") else None

        # Rebuild state
        if self._state_router is not None:
            state = self._state_router.with_event_book(prior_events)
        else:
            state = None

        # Get sequence number
        seq = len(prior_events.pages) if prior_events else 0

        if not request.command.pages:
            raise ValueError("No command pages")

        command_any = request.command.pages[0].command
        type_url = command_any.type_url

        # Find handler
        for full_name, cmd_type, handler in self._handlers:
            if type_url == TYPE_URL_PREFIX + full_name:
                cmd = cmd_type()
                command_any.Unpack(cmd)
                event = handler(cmd, state, seq)

                # Pack event into EventBook
                event_any = AnyProto()
                event_any.Pack(event, type_url_prefix="type.googleapis.com/")

                event_book = types.EventBook(
                    pages=[
                        types.EventPage(
                            header=types.PageHeader(sequence=seq), event=event_any
                        )
                    ]
                )
                return command_handler.BusinessResponse(events=event_book)

        # Unknown command
        raise ValueError(f"Unknown command type: {type_url}")
