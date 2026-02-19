"""Tests for Saga ABC and @reacts_to decorator.

Tests both OO (class-based) and function-based (router) patterns.
Uses consistent domains: order, inventory, fulfillment.
"""

import pytest
from google.protobuf import any_pb2

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.saga import Saga, reacts_to
from angzarr_client.router import EventRouter, event_handler

from .fixtures import (
    CreateShipment,
    OrderCompleted,
    ReserveStock,
    StockReserved,
)


# =============================================================================
# Additional fixture for type hint mismatch testing
# =============================================================================


class AnotherEvent:
    """Another fake event for type mismatch testing."""

    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.AnotherEvent"})()

    def __init__(self, item_id: str = ""):
        self.item_id = item_id

    def SerializeToString(self, deterministic=None):
        return self.item_id.encode()

    def ParseFromString(self, data: bytes):
        self.item_id = data.decode()


# =============================================================================
# OO Pattern: Saga subclass with @reacts_to
# =============================================================================


class OrderFulfillmentSaga(Saga):
    """Saga bridging order → fulfillment domain.

    Uses OO pattern with @reacts_to decorator.
    """

    name = "saga-order-fulfillment"
    input_domain = "order"
    output_domain = "fulfillment"

    @reacts_to(OrderCompleted)
    def handle_completed(self, event: OrderCompleted) -> CreateShipment:
        return CreateShipment(order_id=event.order_id, address="default")


class FulfillmentInventorySaga(Saga):
    """Saga bridging fulfillment → inventory domain."""

    name = "saga-fulfillment-inventory"
    input_domain = "inventory"
    output_domain = "order"

    @reacts_to(StockReserved)
    def handle_reserved(self, event: StockReserved) -> tuple:
        # Return multiple commands
        return (
            ReserveStock(order_id=f"{event.order_id}-1", sku=event.sku, quantity=1),
            ReserveStock(order_id=f"{event.order_id}-2", sku=event.sku, quantity=1),
        )


class NoopSaga(Saga):
    """Saga that returns None (no command)."""

    name = "saga-noop"
    input_domain = "order"
    output_domain = "fulfillment"

    @reacts_to(OrderCompleted)
    def handle_completed(self, event: OrderCompleted) -> None:
        return None


# =============================================================================
# Function-based Pattern: EventRouter with @event_handler
# =============================================================================


def build_order_fulfillment_router() -> EventRouter:
    """Build function-based saga-style router.

    Demonstrates same logic as OrderFulfillmentSaga but with router pattern.
    """
    router = EventRouter("saga-order-fulfillment-fn").domain("order")

    @event_handler(OrderCompleted)
    def handle_completed(
        event: OrderCompleted, root: bytes, correlation_id: str, destinations: list
    ) -> list[types.CommandBook]:
        cmd = CreateShipment(order_id=event.order_id, address="default")
        cmd_any = any_pb2.Any()
        cmd_any.Pack(cmd)
        return [
            types.CommandBook(
                cover=types.Cover(domain="fulfillment", correlation_id=correlation_id),
                pages=[types.CommandPage(command=cmd_any)],
            )
        ]

    router.on(handle_completed)
    return router


def build_inventory_router() -> EventRouter:
    """Function-based router for inventory events."""
    router = EventRouter("saga-inventory-order-fn").domain("inventory")

    @event_handler(StockReserved)
    def handle_reserved(
        event: StockReserved, root: bytes, correlation_id: str, destinations: list
    ) -> list[types.CommandBook]:
        # Return multiple commands
        cmd1 = ReserveStock(order_id=f"{event.order_id}-1", sku=event.sku, quantity=1)
        cmd2 = ReserveStock(order_id=f"{event.order_id}-2", sku=event.sku, quantity=1)

        result = []
        for cmd in [cmd1, cmd2]:
            cmd_any = any_pb2.Any()
            cmd_any.Pack(cmd)
            result.append(
                types.CommandBook(
                    cover=types.Cover(domain="order", correlation_id=correlation_id),
                    pages=[types.CommandPage(command=cmd_any)],
                )
            )
        return result

    router.on(handle_reserved)
    return router


# =============================================================================
# Tests for @reacts_to decorator
# =============================================================================


class TestReactsToDecorator:
    def test_decorator_marks_handler(self):
        method = OrderFulfillmentSaga.handle_completed
        assert hasattr(method, "_is_handler")
        assert method._is_handler is True
        assert method._event_type == OrderCompleted

    def test_decorator_validates_missing_param(self):
        with pytest.raises(TypeError, match="must have cmd parameter"):

            @reacts_to(OrderCompleted)
            def bad_method(self):
                pass

    def test_decorator_validates_missing_type_hint(self):
        with pytest.raises(TypeError, match="missing type hint"):

            @reacts_to(OrderCompleted)
            def bad_method(self, event):
                pass

    def test_decorator_validates_type_hint_mismatch(self):
        with pytest.raises(TypeError, match="doesn't match type hint"):

            @reacts_to(OrderCompleted)
            def bad_method(self, event: AnotherEvent):
                pass

    def test_decorator_preserves_function_name(self):
        method = OrderFulfillmentSaga.handle_completed
        assert method.__name__ == "handle_completed"


# =============================================================================
# Tests for Saga subclass validation
# =============================================================================


class TestSagaSubclassValidation:
    def test_missing_name_raises(self):
        with pytest.raises(TypeError, match="must define 'name'"):

            class BadSaga(Saga):
                input_domain = "order"
                output_domain = "fulfillment"

                @reacts_to(OrderCompleted)
                def handle(self, event: OrderCompleted):
                    pass

    def test_missing_input_domain_raises(self):
        with pytest.raises(TypeError, match="must define 'input_domain'"):

            class BadSaga(Saga):
                name = "bad-saga"
                output_domain = "fulfillment"

                @reacts_to(OrderCompleted)
                def handle(self, event: OrderCompleted):
                    pass

    def test_missing_output_domain_raises(self):
        with pytest.raises(TypeError, match="must define 'output_domain'"):

            class BadSaga(Saga):
                name = "bad-saga"
                input_domain = "order"

                @reacts_to(OrderCompleted)
                def handle(self, event: OrderCompleted):
                    pass

    def test_duplicate_handler_raises(self):
        with pytest.raises(TypeError, match="duplicate handler"):

            class BadSaga(Saga):
                name = "bad-saga"
                input_domain = "order"
                output_domain = "fulfillment"

                @reacts_to(OrderCompleted)
                def handle_one(self, event: OrderCompleted):
                    pass

                @reacts_to(OrderCompleted)
                def handle_two(self, event: OrderCompleted):
                    pass


# =============================================================================
# Tests for OO pattern dispatch
# =============================================================================


class TestSagaDispatch:
    def test_dispatch_finds_handler(self):
        saga = OrderFulfillmentSaga()
        event = OrderCompleted(order_id="order-123", shipped_at="2024-01-15")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        commands = saga.dispatch(event_any, b"\x01\x02", "corr-1")

        assert len(commands) == 1
        assert commands[0].cover.domain == "fulfillment"
        assert commands[0].cover.correlation_id == "corr-1"

    def test_dispatch_unknown_event_returns_empty(self):
        saga = OrderFulfillmentSaga()
        event_any = any_pb2.Any(type_url="test.UnknownEvent", value=b"")

        commands = saga.dispatch(event_any)

        assert commands == []

    def test_dispatch_multiple_commands(self):
        saga = FulfillmentInventorySaga()
        event = StockReserved(order_id="order-456", sku="SKU-A", quantity=10)
        event_any = any_pb2.Any()
        event_any.Pack(event)

        commands = saga.dispatch(event_any)

        assert len(commands) == 2
        assert commands[0].cover.domain == "order"
        assert commands[1].cover.domain == "order"

    def test_dispatch_noop_returns_empty(self):
        saga = NoopSaga()
        event = OrderCompleted(order_id="order-789")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        commands = saga.dispatch(event_any)

        assert commands == []


# =============================================================================
# Tests for OO pattern execute() class method
# =============================================================================


class TestSagaExecute:
    def test_execute_processes_event_book(self):
        event = OrderCompleted(order_id="order-123")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        source = types.EventBook(
            cover=types.Cover(
                domain="order",
                root=types.UUID(value=b"\x01\x02\x03"),
                correlation_id="corr-abc",
            ),
            pages=[types.EventPage(event=event_any)],
        )

        commands = OrderFulfillmentSaga.execute(source)

        assert len(commands) == 1
        assert commands[0].cover.domain == "fulfillment"
        assert commands[0].cover.correlation_id == "corr-abc"

    def test_execute_multiple_events(self):
        event1 = OrderCompleted(order_id="order-1")
        event2 = OrderCompleted(order_id="order-2")
        event_any1 = any_pb2.Any()
        event_any1.Pack(event1)
        event_any2 = any_pb2.Any()
        event_any2.Pack(event2)

        source = types.EventBook(
            pages=[
                types.EventPage(event=event_any1),
                types.EventPage(event=event_any2),
            ],
        )

        commands = OrderFulfillmentSaga.execute(source)

        assert len(commands) == 2

    def test_execute_skips_unhandled_events(self):
        handled = OrderCompleted(order_id="handled")
        unhandled_any = any_pb2.Any(type_url="test.Unhandled", value=b"")
        handled_any = any_pb2.Any()
        handled_any.Pack(handled)

        source = types.EventBook(
            pages=[
                types.EventPage(event=unhandled_any),
                types.EventPage(event=handled_any),
            ],
        )

        commands = OrderFulfillmentSaga.execute(source)

        assert len(commands) == 1


# =============================================================================
# =============================================================================
# Tests for function-based pattern (router)
# =============================================================================


class TestFunctionBasedRouter:
    def test_router_dispatch_order_completed(self):
        router = build_order_fulfillment_router()

        event = OrderCompleted(order_id="order-fn-123")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        source = types.EventBook(
            cover=types.Cover(domain="order", correlation_id="corr-fn-1"),
            pages=[types.EventPage(event=event_any)],
        )

        commands = router.dispatch(source)

        assert len(commands) == 1
        assert commands[0].cover.domain == "fulfillment"
        assert commands[0].cover.correlation_id == "corr-fn-1"

    def test_router_dispatch_multiple_commands(self):
        router = build_inventory_router()

        event = StockReserved(order_id="order-fn-456", sku="SKU-X", quantity=5)
        event_any = any_pb2.Any()
        event_any.Pack(event)

        source = types.EventBook(
            cover=types.Cover(domain="inventory", correlation_id="corr-fn-2"),
            pages=[types.EventPage(event=event_any)],
        )

        commands = router.dispatch(source)

        assert len(commands) == 2
        assert commands[0].cover.domain == "order"
        assert commands[1].cover.domain == "order"

# =============================================================================
# Tests comparing both patterns produce equivalent output
# =============================================================================


class TestPatternEquivalence:
    """Verify OO and function-based patterns produce equivalent results."""

    def test_same_output_for_order_completed(self):
        event = OrderCompleted(order_id="order-eq")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        # OO pattern
        saga = OrderFulfillmentSaga()
        oo_commands = saga.dispatch(event_any, b"\x01", "corr-eq")

        # Function pattern
        router = build_order_fulfillment_router()
        source = types.EventBook(
            cover=types.Cover(domain="order", correlation_id="corr-eq"),
            pages=[types.EventPage(event=event_any)],
        )
        fn_commands = router.dispatch(source)

        # Both produce one command to fulfillment domain
        assert len(oo_commands) == len(fn_commands) == 1
        assert oo_commands[0].cover.domain == fn_commands[0].cover.domain == "fulfillment"
        assert oo_commands[0].cover.correlation_id == fn_commands[0].cover.correlation_id == "corr-eq"
