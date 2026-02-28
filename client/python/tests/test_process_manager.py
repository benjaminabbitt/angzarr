"""Tests for ProcessManager ABC and @handles decorator with input_domain.

Tests both OO (class-based) and protocol-based (router) patterns.
Uses consistent domains: order, inventory, fulfillment.
"""

from dataclasses import dataclass

import pytest
from google.protobuf import any_pb2

from angzarr_client.handler_protocols import (
    ProcessManagerDomainHandler,
    ProcessManagerResponse,
)
from angzarr_client.process_manager import ProcessManager, handles
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import ProcessManagerRouter
from angzarr_client.state_builder import StateRouter

from .fixtures import (
    CreateShipment,
    OrderCompleted,
    OrderCreated,
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
# OO Pattern: ProcessManager subclass with @handles
# =============================================================================


class OrderWorkflowPM(ProcessManager[OrderWorkflowState]):
    """Process manager coordinating order → inventory → fulfillment workflow.

    Uses OO pattern with @handles decorator.
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

    @handles(OrderCreated, input_domain="order", output_domain="inventory")
    def on_order_created(self, event: OrderCreated) -> ReserveStock:
        return ReserveStock(order_id=event.order_id, sku="default", quantity=1)

    @handles(StockReserved, input_domain="inventory", output_domain="fulfillment")
    def on_stock_reserved(self, event: StockReserved) -> CreateShipment:
        state = self._get_state()
        return CreateShipment(
            order_id=event.order_id, address=f"customer-{state.customer_id}"
        )


class NoopPM(ProcessManager[OrderWorkflowState]):
    """Process manager that returns None (no command)."""

    name = "noop-pm"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    def _apply_event(self, state: OrderWorkflowState, event_any: any_pb2.Any) -> None:
        pass

    @handles(OrderCreated, input_domain="order")
    def on_order_created(self, event: OrderCreated) -> None:
        return None


class MultiCommandPM(ProcessManager[OrderWorkflowState]):
    """Process manager that returns multiple commands."""

    name = "multi-pm"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    def _apply_event(self, state: OrderWorkflowState, event_any: any_pb2.Any) -> None:
        pass

    @handles(OrderCreated, input_domain="order", output_domain="inventory")
    def on_order_created(self, event: OrderCreated) -> tuple:
        return (
            ReserveStock(order_id=event.order_id, sku="item-1", quantity=1),
            ReserveStock(order_id=event.order_id, sku="item-2", quantity=2),
        )


# =============================================================================
# Protocol-based Pattern: ProcessManagerRouter with ProcessManagerDomainHandler
# =============================================================================


@dataclass
class RouterPMState:
    """State for protocol-based PM routers."""

    order_id: str = ""


class _FakePMEvent:
    """Fake event type for StateRouter registration."""

    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakePMEvent"})()


def apply_router_pm_event(state: RouterPMState, event: _FakePMEvent) -> None:
    """Apply events to router PM state (no-op for these tests)."""
    pass


ROUTER_PM_STATE_ROUTER = StateRouter(RouterPMState).on(
    _FakePMEvent, apply_router_pm_event
)


class OrderWorkflowPMHandler(ProcessManagerDomainHandler[RouterPMState]):
    """Protocol-based PM handler for order domain events."""

    def event_types(self) -> list[str]:
        return ["OrderCreated"]

    def state_router(self) -> StateRouter[RouterPMState]:
        return ROUTER_PM_STATE_ROUTER

    def handle(
        self,
        trigger: types.EventBook,
        state: RouterPMState,
        event: any_pb2.Any,
        destinations: list[types.EventBook],
    ) -> ProcessManagerResponse:
        order_created = OrderCreated()
        order_created.ParseFromString(event.value)
        cmd = ReserveStock(order_id=order_created.order_id, sku="default", quantity=1)
        cmd_any = any_pb2.Any()
        cmd_any.Pack(cmd)
        return ProcessManagerResponse(
            commands=[
                types.CommandBook(
                    cover=types.Cover(
                        domain="inventory",
                        correlation_id=trigger.cover.correlation_id,
                    ),
                    pages=[types.CommandPage(command=cmd_any)],
                )
            ]
        )


class InventoryPMHandler(ProcessManagerDomainHandler[RouterPMState]):
    """Protocol-based PM handler for inventory domain events."""

    def event_types(self) -> list[str]:
        return ["StockReserved"]

    def state_router(self) -> StateRouter[RouterPMState]:
        return ROUTER_PM_STATE_ROUTER

    def handle(
        self,
        trigger: types.EventBook,
        state: RouterPMState,
        event: any_pb2.Any,
        destinations: list[types.EventBook],
    ) -> ProcessManagerResponse:
        stock_reserved = StockReserved()
        stock_reserved.ParseFromString(event.value)
        cmd = CreateShipment(
            order_id=stock_reserved.order_id, address="default-address"
        )
        cmd_any = any_pb2.Any()
        cmd_any.Pack(cmd)
        return ProcessManagerResponse(
            commands=[
                types.CommandBook(
                    cover=types.Cover(
                        domain="fulfillment",
                        correlation_id=trigger.cover.correlation_id,
                    ),
                    pages=[types.CommandPage(command=cmd_any)],
                )
            ]
        )


def build_order_workflow_router() -> ProcessManagerRouter[RouterPMState]:
    """Build protocol-based PM router.

    Demonstrates same logic as OrderWorkflowPM but with router pattern.
    """
    handler = OrderWorkflowPMHandler()
    return ProcessManagerRouter("order-workflow-fn", "order", handler)


def build_inventory_pm_router() -> ProcessManagerRouter[RouterPMState]:
    """Separate router for inventory domain events."""
    handler = InventoryPMHandler()
    return ProcessManagerRouter("order-workflow-fn", "inventory", handler)


# =============================================================================
# Tests for @handles decorator with input_domain
# =============================================================================


class TestHandlesWithInputDomain:
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

                @handles(OrderCreated, input_domain="order")
                def handle_one(self, event: OrderCreated):
                    pass

                @handles(OrderCreated, input_domain="order")
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

        response = OrderWorkflowPM.handle(trigger, process_state)

        assert len(response.commands) == 1
        assert response.commands[0].cover.domain == "inventory"
        assert response.commands[0].cover.correlation_id == "corr-abc"


# =============================================================================
# Tests for protocol-based pattern (router)
# =============================================================================


class TestProtocolBasedRouter:
    def test_router_dispatch_order_created(self):
        router = build_order_workflow_router()

        event = OrderCreated(order_id="order-fn-123")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        trigger = types.EventBook(
            cover=types.Cover(domain="order", correlation_id="corr-fn-1"),
            pages=[types.EventPage(event=event_any)],
        )
        pm_state = types.EventBook()

        response = router.dispatch(trigger, pm_state)
        commands = list(response.commands)

        assert len(commands) == 1
        assert commands[0].cover.domain == "inventory"
        assert commands[0].cover.correlation_id == "corr-fn-1"

    def test_router_dispatch_stock_reserved(self):
        router = build_inventory_pm_router()

        event = StockReserved(order_id="order-fn-456", sku="item", quantity=1)
        event_any = any_pb2.Any()
        event_any.Pack(event)

        trigger = types.EventBook(
            cover=types.Cover(domain="inventory", correlation_id="corr-fn-2"),
            pages=[types.EventPage(event=event_any)],
        )
        pm_state = types.EventBook()

        response = router.dispatch(trigger, pm_state)
        commands = list(response.commands)

        assert len(commands) == 1
        assert commands[0].cover.domain == "fulfillment"


# =============================================================================
# Tests comparing both patterns produce equivalent output
# =============================================================================


class TestPatternEquivalence:
    """Verify OO and protocol-based patterns produce equivalent results."""

    def test_same_output_for_order_created(self):
        event = OrderCreated(order_id="order-eq-1", customer_id="cust-eq")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        # OO pattern
        pm_instance = OrderWorkflowPM()
        oo_commands = pm_instance.dispatch(event_any, b"\x01", "corr-eq")

        # Protocol pattern
        router = build_order_workflow_router()
        trigger = types.EventBook(
            cover=types.Cover(domain="order", correlation_id="corr-eq"),
            pages=[types.EventPage(event=event_any)],
        )
        pm_state = types.EventBook()
        response = router.dispatch(trigger, pm_state)
        router_commands = list(response.commands)

        # Both produce one command to inventory domain
        assert len(oo_commands) == len(router_commands) == 1
        assert (
            oo_commands[0].cover.domain
            == router_commands[0].cover.domain
            == "inventory"
        )
        assert (
            oo_commands[0].cover.correlation_id
            == router_commands[0].cover.correlation_id
            == "corr-eq"
        )


# =============================================================================
# Tests for ProcessManager fact output
# =============================================================================


class PMWithFacts(ProcessManager[OrderWorkflowState]):
    """Process manager that emits facts."""

    name = "pm-with-facts"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    def _apply_event(self, state: OrderWorkflowState, event_any: any_pb2.Any) -> None:
        pass

    @handles(OrderCreated, input_domain="order", output_domain="inventory")
    def on_order_created(self, event: OrderCreated) -> ReserveStock:
        # Emit a fact to another aggregate
        self.emit_fact(
            types.EventBook(
                cover=types.Cover(domain="analytics", correlation_id="test"),
                pages=[
                    types.EventPage(event=any_pb2.Any(type_url="test/OrderAnalytics"))
                ],
            )
        )
        return ReserveStock(order_id=event.order_id, sku="default", quantity=1)


class TestProcessManagerFactOutput:
    def test_emit_fact_accumulates_facts(self):
        pm = PMWithFacts()
        event = OrderCreated(order_id="order-123")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        pm.dispatch(event_any, b"\x01\x02", "corr-1")

        assert len(pm._facts) == 1
        assert pm._facts[0].cover.domain == "analytics"

    def test_handle_returns_facts_in_response(self):
        event = OrderCreated(order_id="order-123")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        trigger = types.EventBook(
            cover=types.Cover(domain="order", correlation_id="corr-abc"),
            pages=[types.EventPage(event=event_any)],
        )
        process_state = types.EventBook()

        response = PMWithFacts.handle(trigger, process_state)

        assert len(response.commands) == 1
        assert len(response.facts) == 1
        assert response.facts[0].cover.domain == "analytics"

    def test_pm_init_resets_facts(self):
        # Each PM instance should start with empty facts
        pm1 = PMWithFacts()
        pm1.emit_fact(types.EventBook(cover=types.Cover(domain="test")))
        assert len(pm1._facts) == 1

        pm2 = PMWithFacts()
        assert len(pm2._facts) == 0


# =============================================================================
# Tests for @applies decorator in ProcessManager
# =============================================================================

from angzarr_client.process_manager import applies


class PMWithApplies(ProcessManager[OrderWorkflowState]):
    """Process manager using @applies decorator pattern."""

    name = "pm-with-applies"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    @applies(OrderCreated)
    def apply_order_created(
        self, state: OrderWorkflowState, event: OrderCreated
    ) -> None:
        state.order_id = event.order_id
        state.customer_id = event.customer_id

    @applies(StockReserved)
    def apply_stock_reserved(
        self, state: OrderWorkflowState, event: StockReserved
    ) -> None:
        state.inventory_reserved = True

    @handles(OrderCreated, input_domain="order", output_domain="inventory")
    def on_order_created(self, event: OrderCreated) -> ReserveStock:
        return ReserveStock(order_id=event.order_id, sku="default", quantity=1)


class TestProcessManagerAppliesDecorator:
    """Tests for @applies decorator support in ProcessManager."""

    def test_applier_table_populated(self):
        """Verify @applies methods are discovered."""
        assert "OrderCreated" in PMWithApplies._applier_table
        assert "StockReserved" in PMWithApplies._applier_table
        assert len(PMWithApplies._applier_table) == 2

    def test_apply_event_dispatches_to_applier(self):
        """Verify _apply_event routes to correct @applies method."""
        pm = PMWithApplies()
        state = pm._create_empty_state()

        event = OrderCreated(order_id="test-123", customer_id="cust-456")
        event_any = any_pb2.Any()
        event_any.Pack(event)

        pm._apply_event(state, event_any)

        assert state.order_id == "test-123"
        assert state.customer_id == "cust-456"

    def test_unknown_event_silently_ignored(self):
        """Verify unknown events don't raise errors."""
        pm = PMWithApplies()
        state = pm._create_empty_state()

        event_any = any_pb2.Any(type_url="test.UnknownEventType", value=b"")
        pm._apply_event(state, event_any)  # Should not raise

        # State unchanged
        assert state.order_id == ""

    def test_state_rebuilt_with_applies(self):
        """Verify state rebuild works with @applies methods."""
        # Create event book with prior events
        event1 = OrderCreated(order_id="order-rebuild", customer_id="cust-rebuild")
        event1_any = any_pb2.Any()
        event1_any.Pack(event1)

        event2 = StockReserved(order_id="order-rebuild", sku="item", quantity=1)
        event2_any = any_pb2.Any()
        event2_any.Pack(event2)

        event_book = types.EventBook(
            pages=[
                types.EventPage(event=event1_any),
                types.EventPage(event=event2_any),
            ]
        )

        pm = PMWithApplies(event_book)
        state = pm._get_state()

        assert state.order_id == "order-rebuild"
        assert state.customer_id == "cust-rebuild"
        assert state.inventory_reserved is True

    def test_duplicate_applier_raises(self):
        """Verify duplicate @applies for same event type raises TypeError."""
        with pytest.raises(TypeError, match="duplicate applier"):

            class BadPM(ProcessManager[OrderWorkflowState]):
                name = "bad-applier-pm"

                def _create_empty_state(self):
                    return OrderWorkflowState()

                @applies(OrderCreated)
                def apply_one(self, state: OrderWorkflowState, event: OrderCreated):
                    pass

                @applies(OrderCreated)
                def apply_two(self, state: OrderWorkflowState, event: OrderCreated):
                    pass

    def test_missing_applier_and_override_raises(self):
        """Verify PM without @applies or _apply_event override raises."""
        with pytest.raises(NotImplementedError, match="must either define @applies"):

            class MinimalPM(ProcessManager[OrderWorkflowState]):
                name = "minimal-pm"

                def _create_empty_state(self):
                    return OrderWorkflowState()

            pm = MinimalPM()
            state = pm._create_empty_state()
            event_any = any_pb2.Any(type_url="test.SomeEvent", value=b"")
            pm._apply_event(state, event_any)
