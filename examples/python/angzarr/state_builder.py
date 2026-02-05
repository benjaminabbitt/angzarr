"""StateBuilder for declarative event handler registration.

Replaces manual if/elif chains in rebuild_state functions.
Mirrors CommandRouter's pattern of registering handlers by type suffix.
"""

from __future__ import annotations

from typing import Callable, Generic, TypeVar

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types

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
