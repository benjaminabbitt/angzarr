"""StateBuilder and StateRouter for declarative event handler registration.

Replaces manual if/elif chains in rebuild_state functions.

StateBuilder: Low-level, handlers receive raw Any and do their own unpacking.
StateRouter: Higher-level, auto-unpacks events to typed handlers.
"""

from __future__ import annotations

from typing import Callable, Generic, TypeVar, List

from google.protobuf.any_pb2 import Any as AnyProto

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

    def on(self, type_suffix: str, apply: StateApplier[S]) -> StateBuilder[S]:
        """Register an event applier for a type_url suffix.

        The applier function is responsible for decoding the event.

        Args:
            type_suffix: Event type_url suffix to match.
            apply: Function receiving (state, raw_any_proto).
        """
        self._appliers.append((type_suffix, apply))
        return self

    def apply(self, state: S, event: AnyProto) -> None:
        """Apply a single event to state using registered handlers.

        Useful for applying newly-created events to current state
        without going through full EventBook reconstruction.
        """
        if event is None:
            return
        for suffix, applier in self._appliers:
            if event.type_url.endswith(suffix):
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
        self._handlers: List[tuple[str, type, Callable[[S, object], None]]] = []
        self._snapshot_loader: SnapshotLoader[S] | None = None

    def on(self, event_type: type, handler: Callable[[S, object], None]) -> StateRouter[S]:
        """Register a handler for an event type.

        The handler receives typed events (auto-unpacked from Any).

        Args:
            event_type: The protobuf event class to handle.
            handler: Function(state, event) that mutates state.

        Returns:
            Self for chaining.
        """
        suffix = event_type.__name__
        self._handlers.append((suffix, event_type, handler))
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
        for suffix, event_type, handler in self._handlers:
            if type_url.endswith(suffix):
                event = event_type()
                event_any.Unpack(event)
                handler(state, event)
                return

        # Unknown event type - silently ignore (forward compatibility)
