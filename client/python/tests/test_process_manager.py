"""Tests for ProcessManager ABC and @reacts_to decorator with input_domain.

Tests both OO (class-based) and function-based (router) patterns.
Uses consistent domains: order, inventory, fulfillment.
"""

from dataclasses import dataclass

import pytest
from google.protobuf import any_pb2

from angzarr_client.process_manager import ProcessManager, reacts_to
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import EventRouter, event_handler

from .fixtures import (
    CreateShipment,
    OrderCreated,
    OrderCompleted,
    ReserveStock,
    StockReserved,
)


# =============================================================================
# Shared state for OO pattern
# =============================================================================


@dataclass
class OrderWorkflowState:
    """State for order workflow process manager."""

    order_id: str = ""
    customer_id: str = ""
    inventory_reserved: bool = False
    shipment_created: bool = False


# =============================================================================
# OO Pattern: ProcessManager subclass with @reacts_to
# =============================================================================


class OrderWorkflowPM(ProcessManager[OrderWorkflowState]):
    """Process manager coordinating order → inventory → fulfillment workflow.

    Uses OO pattern with @reacts_to decorator.
    """

    name = "order-workflow"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    def _apply_event(self, state: OrderWorkflowState, event_any: any_pb2.Any) -> None:
        if event_any.type_url.endswith("OrderCreated"):
            event = OrderCreated()
            event.ParseFromString(event_any.value)
            state.order_id = event.order_id
            state.customer_id = event.customer_id
        elif event_any.type_url.endswith("StockReserved"):
            state.inventory_reserved = True

    @reacts_to(OrderCreated, input_domain="order", output_domain="inventory")
    def on_order_created(self, event: OrderCreated) -> ReserveStock:
        return ReserveStock(order_id=event.order_id, sku="default", quantity=1)

    @reacts_to(StockReserved, input_domain="inventory", output_domain="fulfillment")
    def on_stock_reserved(self, event: StockReserved) -> CreateShipment:
        state = self._get_state()
        return CreateShipment(order_id=event.order_id, address=f"customer-{state.customer_id}")


class NoopPM(ProcessManager[OrderWorkflowState]):
    """Process manager that returns None (no command)."""

    name = "noop-pm"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    def _apply_event(self, state: OrderWorkflowState, event_any: any_pb2.Any) -> None:
        pass

    @reacts_to(OrderCreated, input_domain="order")
    def on_order_created(self, event: OrderCreated) -> None:
        return None


class MultiCommandPM(ProcessManager[OrderWorkflowState]):
    """Process manager that returns multiple commands."""

    name = "multi-pm"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    def _apply_event(self, state: OrderWorkflowState, event_any: any_pb2.Any) -> None:
        pass

    @reacts_to(OrderCreated, input_domain="order", output_domain="inventory")
    def on_order_created(self, event: OrderCreated) -> tuple:
        return (
            ReserveStock(order_id=event.order_id, sku="item-1", quantity=1),
            ReserveStock(order_id=event.order_id, sku="item-2", quantity=2),
        )


# =============================================================================
# Function-based Pattern: EventRouter with @event_handler
# =============================================================================


def build_order_workflow_router() -> EventRouter:
    """Build function-based PM-style router.

    Demonstrates same logic as OrderWorkflowPM but with router pattern.
    """
    router = EventRouter("order-workflow-fn").domain("order")

    @event_handler(OrderCreated)
    def handle_order_created(
        event: OrderCreated, root: bytes, correlation_id: str, destinations: list
    ) -> list[types.CommandBook]:
        cmd = ReserveStock(order_id=event.order_id, sku="default", quantity=1)
        cmd_any = any_pb2.Any()
        cmd_any.Pack(cmd)
        return [
            types.CommandBook(
                cover=types.Cover(domain="inventory", correlation_id=correlation_id),
                pages=[types.CommandPage(command=cmd_any)],
            )
        ]

    router.on(handle_order_created)
    return router


def build_inventory_router() -> EventRouter:
    """Separate router for inventory domain events."""
    router = EventRouter("order-workflow-fn").domain("inventory")

    @event_handler(StockReserved)
    def handle_stock_reserved(
        event: StockReserved, root: bytes, correlation_id: str, destinations: list
    ) -> list[types.CommandBook]:
        cmd = CreateShipment(order_id=event.order_id, address="default-address")
        cmd_any = any_pb2.Any()
        cmd_any.Pack(cmd)
        return [
            types.CommandBook(
                cover=types.Cover(domain="fulfillment", correlation_id=correlation_id),
                pages=[types.CommandPage(command=cmd_any)],
            )
        ]

    router.on(handle_stock_reserved)
    return router


# =============================================================================
# Tests for @reacts_to decorator with input_domain
# =============================================================================


class TestReactsToWithInputDomain:
    def test_decorator_stores_input_domain(self):
        method = OrderWorkflowPM.on_order_created
        assert method._input_domain == "order"

    def test_decorator_stores_output_domain(self):
        method = OrderWorkflowPM.on_order_created
        assert method._output_domain == "inventory"

    def test_decorator_optional_output_domain(self):
        method = NoopPM.on_order_created
        assert method._output_domain is None


# =============================================================================
# Tests for ProcessManager subclass validation
# =============================================================================


class TestProcessManagerValidation:
    def test_missing_name_raises(self):
        with pytest.raises(TypeError, match="must define 'name'"):

            class BadPM(ProcessManager[OrderWorkflowState]):
                def _create_empty_state(self):
                    return OrderWorkflowState()

                def _apply_event(self, state, event_any):
                    pass

    def test_duplicate_handler_raises(self):
        with pytest.raises(TypeError, match="duplicate handler"):

            class BadPM(ProcessManager[OrderWorkflowState]):
                name = "bad-pm"

                def _create_empty_state(self):
                    return OrderWorkflowState()

                def _apply_event(self, state, event_any):
                    pass

                @reacts_to(OrderCreated, input_domain="order")
                def handle_one(self, event: OrderCreated):
                    pass

                @reacts_to(OrderCreated, input_domain="order")
                def handle_two(self, event: OrderCreated):
                    pass


# =============================================================================
# Tests for OO pattern dispatch
# =============================================================================


class TestProcessManagerDispatch:
    def test_dispatch_finds_handler(self):
        pm = OrderWorkflowPM()
        event = OrderCreated(order_id="order-123", customer_id="cust-1")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        commands = pm.dispatch(event_any, b"\x01\x02", "corr-1")

        assert len(commands) == 1
        assert commands[0].cover.domain == "inventory"
        assert commands[0].cover.correlation_id == "corr-1"

    def test_dispatch_unknown_event_returns_empty(self):
        pm = OrderWorkflowPM()
        event_any = any_pb2.Any(type_url="test.UnknownEvent", value=b"")

        commands = pm.dispatch(event_any)

        assert commands == []

    def test_dispatch_multiple_commands(self):
        pm = MultiCommandPM()
        event = OrderCreated(order_id="order-456")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        commands = pm.dispatch(event_any)

        assert len(commands) == 2
        assert commands[0].cover.domain == "inventory"
        assert commands[1].cover.domain == "inventory"

    def test_dispatch_noop_returns_empty(self):
        pm = NoopPM()
        event = OrderCreated(order_id="order-789")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        commands = pm.dispatch(event_any)

        assert commands == []


# =============================================================================
# Tests for OO pattern state rebuilding
# =============================================================================


class TestProcessManagerState:
    def test_state_rebuilt_from_event_book(self):
        # Build prior events
        event = OrderCreated(order_id="order-123", customer_id="cust-456")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        prior_events = types.EventBook(
            pages=[types.EventPage(event=event_any)],
        )

        pm = OrderWorkflowPM(prior_events)
        state = pm._get_state()

        assert state.order_id == "order-123"
        assert state.customer_id == "cust-456"

    def test_handler_uses_state(self):
        # Prior events establish state
        order_event = OrderCreated(order_id="order-123", customer_id="cust-999")
        order_any = any_pb2.Any()
        order_any.Pack(order_event)

        prior_events = types.EventBook(
            pages=[types.EventPage(event=order_any)],
        )

        pm = OrderWorkflowPM(prior_events)

        # Now process StockReserved which uses state for address
        stock_event = StockReserved(order_id="order-123", sku="item", quantity=1)
        stock_any = any_pb2.Any()
        stock_any.Pack(stock_event)

        commands = pm.dispatch(stock_any)

        assert len(commands) == 1
        # Unpack command to verify address uses customer_id from state
        cmd = CreateShipment()
        commands[0].pages[0].command.Unpack(cmd)
        assert cmd.address == "customer-cust-999"


# =============================================================================
# Tests for OO pattern handle() class method
# =============================================================================


class TestProcessManagerHandle:
    def test_handle_processes_trigger(self):
        event = OrderCreated(order_id="order-123", customer_id="cust-1")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        trigger = types.EventBook(
            cover=types.Cover(
                domain="order",
                correlation_id="corr-abc",
            ),
            pages=[types.EventPage(event=event_any)],
        )
        process_state = types.EventBook()

        commands, new_events = OrderWorkflowPM.handle(trigger, process_state)

        assert len(commands) == 1
        assert commands[0].cover.domain == "inventory"
        assert commands[0].cover.correlation_id == "corr-abc"




# =============================================================================
# Tests for function-based pattern (router)
# =============================================================================


class TestFunctionBasedRouter:
    def test_router_dispatch_order_created(self):
        router = build_order_workflow_router()

        event = OrderCreated(order_id="order-fn-123")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        source = types.EventBook(
            cover=types.Cover(domain="order", correlation_id="corr-fn-1"),
            pages=[types.EventPage(event=event_any)],
        )

        commands = router.dispatch(source)

        assert len(commands) == 1
        assert commands[0].cover.domain == "inventory"
        assert commands[0].cover.correlation_id == "corr-fn-1"

    def test_router_dispatch_stock_reserved(self):
        router = build_inventory_router()

        event = StockReserved(order_id="order-fn-456", sku="item", quantity=1)
        event_any = any_pb2.Any()
        event_any.Pack(event)

        source = types.EventBook(
            cover=types.Cover(domain="inventory", correlation_id="corr-fn-2"),
            pages=[types.EventPage(event=event_any)],
        )

        commands = router.dispatch(source)

        assert len(commands) == 1
        assert commands[0].cover.domain == "fulfillment"

# =============================================================================
# Tests comparing both patterns produce equivalent output
# =============================================================================


class TestPatternEquivalence:
    """Verify OO and function-based patterns produce equivalent results."""

    def test_same_output_for_order_created(self):
        event = OrderCreated(order_id="order-eq-1", customer_id="cust-eq")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        # OO pattern
        pm = OrderWorkflowPM()
        oo_commands = pm.dispatch(event_any, b"\x01", "corr-eq")

        # Function pattern
        router = build_order_workflow_router()
        source = types.EventBook(
            cover=types.Cover(domain="order", correlation_id="corr-eq"),
            pages=[types.EventPage(event=event_any)],
        )
        fn_commands = router.dispatch(source)

        # Both produce one command to inventory domain
        assert len(oo_commands) == len(fn_commands) == 1
        assert oo_commands[0].cover.domain == fn_commands[0].cover.domain == "inventory"
        assert oo_commands[0].cover.correlation_id == fn_commands[0].cover.correlation_id == "corr-eq"
