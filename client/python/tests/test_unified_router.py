"""Tests for unified router pattern: AggregateRouter, SagaRouter, ProcessManagerRouter, ProjectorRouter.

These routers wrap handler protocol implementations and provide unified routing
with two patterns:
- Single-domain: AggregateRouter, SagaRouter (domain set at construction)
- Multi-domain: ProcessManagerRouter, ProjectorRouter (fluent .domain() method)
"""

from dataclasses import dataclass, field

import pytest
from google.protobuf import any_pb2

from angzarr_client import RejectionHandlerResponse
from angzarr_client.compensation import CompensationContext
from angzarr_client.handler_protocols import (
    AggregateDomainHandler,
    ProcessManagerDomainHandler,
    ProcessManagerResponse,
    ProjectorDomainHandler,
    SagaDomainHandler,
)
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import (
    NOTIFICATION_TYPE_URL,
    AggregateRouter,
    ProcessManagerRouter,
    ProjectorRouter,
    SagaRouter,
    _extract_rejection_key,
    _next_sequence,
)
from angzarr_client.state_builder import StateRouter

from .fixtures import (
    DepositFunds,
    FundsDeposited,
    PlayerRegistered,
    StockReserved,
    StockUpdated,
)

# =============================================================================
# Test State Types
# =============================================================================


@dataclass
class PlayerState:
    """Test aggregate state."""

    exists: bool = False
    player_id: str = ""
    bankroll: int = 0


@dataclass
class WorkflowState:
    """Test PM state."""

    steps_completed: list = field(default_factory=list)
    failed: bool = False


# =============================================================================
# Test Handler Implementations
# =============================================================================


def apply_player_registered(state: PlayerState, event: PlayerRegistered) -> None:
    state.exists = True
    state.player_id = event.player_id
    state.bankroll = 0


def apply_funds_deposited(state: PlayerState, event: FundsDeposited) -> None:
    state.bankroll = event.new_bankroll


# Build state router for PlayerState
PLAYER_STATE_ROUTER = (
    StateRouter(PlayerState)
    .on(PlayerRegistered, apply_player_registered)
    .on(FundsDeposited, apply_funds_deposited)
)


class MockPlayerHandler(AggregateDomainHandler[PlayerState]):
    """Test aggregate handler implementation."""

    def command_types(self) -> list[str]:
        return ["RegisterPlayer", "DepositFunds"]

    def state_router(self) -> StateRouter[PlayerState]:
        return PLAYER_STATE_ROUTER

    def handle(
        self,
        cmd_book: types.CommandBook,
        payload: any_pb2.Any,
        state: PlayerState,
        seq: int,
    ) -> types.EventBook:
        if payload.type_url.endswith("RegisterPlayer"):
            event = PlayerRegistered(player_id="player_1", display_name="Test")
            event_any = any_pb2.Any()
            event_any.Pack(event)
            return types.EventBook(
                cover=cmd_book.cover,
                pages=[types.EventPage(sequence=seq, event=event_any)],
            )
        elif payload.type_url.endswith("DepositFunds"):
            cmd = DepositFunds()
            payload.Unpack(cmd)
            event = FundsDeposited(new_bankroll=state.bankroll + cmd.amount)
            event_any = any_pb2.Any()
            event_any.Pack(event)
            return types.EventBook(
                cover=cmd_book.cover,
                pages=[types.EventPage(sequence=seq, event=event_any)],
            )
        raise ValueError(f"Unknown command: {payload.type_url}")

    def on_rejected(
        self,
        notification: types.Notification,
        state: PlayerState,
        target_domain: str,
        target_command: str,
    ) -> RejectionHandlerResponse:
        return RejectionHandlerResponse()


class MockSagaHandler(SagaDomainHandler):
    """Test saga handler implementation."""

    def event_types(self) -> list[str]:
        return ["StockUpdated"]

    def prepare(
        self,
        source: types.EventBook,
        event: any_pb2.Any,
    ) -> list[types.Cover]:
        return [types.Cover(domain="inventory", root=source.cover.root)]

    def execute(
        self,
        source: types.EventBook,
        event: any_pb2.Any,
        destinations: list[types.EventBook],
    ) -> list[types.CommandBook]:
        # Create a command targeting inventory domain
        return [
            types.CommandBook(
                cover=types.Cover(
                    domain="inventory",
                    root=source.cover.root,
                    correlation_id=source.cover.correlation_id,
                ),
                pages=[
                    types.CommandPage(command=any_pb2.Any(type_url="test/ReserveStock"))
                ],
            )
        ]


class MockPMInventoryHandler(ProcessManagerDomainHandler[WorkflowState]):
    """Test PM handler for inventory domain."""

    def event_types(self) -> list[str]:
        return ["StockReserved"]

    def prepare(
        self,
        trigger: types.EventBook,
        state: WorkflowState,
        event: any_pb2.Any,
    ) -> list[types.Cover]:
        return []

    def handle(
        self,
        trigger: types.EventBook,
        state: WorkflowState,
        event: any_pb2.Any,
        destinations: list[types.EventBook],
    ) -> ProcessManagerResponse:
        return ProcessManagerResponse(
            commands=[
                types.CommandBook(
                    cover=types.Cover(domain="fulfillment"),
                    pages=[
                        types.CommandPage(
                            command=any_pb2.Any(type_url="test/CreateShipment")
                        )
                    ],
                )
            ]
        )

    def on_rejected(
        self,
        notification: types.Notification,
        state: WorkflowState,
        target_domain: str,
        target_command: str,
    ) -> RejectionHandlerResponse:
        return RejectionHandlerResponse()


class MockPMOrderHandler(ProcessManagerDomainHandler[WorkflowState]):
    """Test PM handler for order domain."""

    def event_types(self) -> list[str]:
        return ["OrderCreated"]

    def prepare(
        self,
        trigger: types.EventBook,
        state: WorkflowState,
        event: any_pb2.Any,
    ) -> list[types.Cover]:
        return [types.Cover(domain="inventory")]

    def handle(
        self,
        trigger: types.EventBook,
        state: WorkflowState,
        event: any_pb2.Any,
        destinations: list[types.EventBook],
    ) -> ProcessManagerResponse:
        return ProcessManagerResponse()

    def on_rejected(
        self,
        notification: types.Notification,
        state: WorkflowState,
        target_domain: str,
        target_command: str,
    ) -> RejectionHandlerResponse:
        return RejectionHandlerResponse()


class MockProjectorHandler(ProjectorDomainHandler):
    """Test projector handler implementation."""

    def event_types(self) -> list[str]:
        return ["StockUpdated", "StockReserved"]

    def project(self, events: types.EventBook) -> types.Projection:
        return types.Projection(projector="test-projector")


# =============================================================================
# Helper Functions
# =============================================================================


def make_contextual_command(
    domain: str, type_suffix: str, prior_events: types.EventBook | None = None
) -> types.ContextualCommand:
    """Create a ContextualCommand for testing."""
    cmd_any = any_pb2.Any(type_url=f"type.googleapis.com/test.{type_suffix}")
    cmd = types.ContextualCommand(
        command=types.CommandBook(
            cover=types.Cover(domain=domain),
            pages=[types.CommandPage(command=cmd_any)],
        ),
    )
    if prior_events is not None:
        cmd.events.CopyFrom(prior_events)
    return cmd


def make_event_book(
    domain: str, event_type: str, correlation_id: str = ""
) -> types.EventBook:
    """Create an EventBook for testing."""
    event_any = any_pb2.Any(type_url=f"type.googleapis.com/test.{event_type}")
    return types.EventBook(
        cover=types.Cover(
            domain=domain,
            root=types.UUID(value=b"\x01\x02\x03"),
            correlation_id=correlation_id,
        ),
        pages=[types.EventPage(event=event_any)],
    )


# =============================================================================
# AggregateRouter Tests
# =============================================================================


class TestAggregateRouter:
    """Tests for AggregateRouter (single-domain, commands -> events)."""

    def test_construction_sets_name_and_domain(self):
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        assert router.name == "player"
        assert router.domain == "player"

    def test_command_types_delegates_to_handler(self):
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        assert router.command_types() == ["RegisterPlayer", "DepositFunds"]

    def test_subscriptions_returns_domain_and_types(self):
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        subs = router.subscriptions()
        assert subs == [("player", ["RegisterPlayer", "DepositFunds"])]

    def test_dispatch_routes_to_handler(self):
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        cmd = make_contextual_command("player", "RegisterPlayer")
        response = router.dispatch(cmd)

        assert response.WhichOneof("result") == "events"
        assert len(response.events.pages) == 1

    def test_dispatch_rebuilds_state_from_prior_events(self):
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        # Create prior events (player registered with deposit)
        reg_event = PlayerRegistered(player_id="p1", display_name="Test")
        reg_any = any_pb2.Any()
        reg_any.Pack(reg_event)

        deposit_event = FundsDeposited(new_bankroll=100)
        dep_any = any_pb2.Any()
        dep_any.Pack(deposit_event)

        prior = types.EventBook(
            cover=types.Cover(domain="player"),
            pages=[
                types.EventPage(sequence=0, event=reg_any),
                types.EventPage(sequence=1, event=dep_any),
            ],
        )

        # Create deposit command
        deposit_cmd = DepositFunds(amount=50)
        cmd_any = any_pb2.Any()
        cmd_any.Pack(deposit_cmd)

        cmd = types.ContextualCommand(
            command=types.CommandBook(
                cover=types.Cover(domain="player"),
                pages=[types.CommandPage(command=cmd_any)],
            ),
        )
        cmd.events.CopyFrom(prior)

        response = router.dispatch(cmd)

        # New bankroll should be 100 + 50 = 150
        assert response.WhichOneof("result") == "events"
        result_event = FundsDeposited()
        response.events.pages[0].event.Unpack(result_event)
        assert result_event.new_bankroll == 150

    def test_dispatch_raises_on_empty_pages(self):
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        cmd = types.ContextualCommand(
            command=types.CommandBook(cover=types.Cover(domain="player")),
        )

        with pytest.raises(ValueError, match="No command pages"):
            router.dispatch(cmd)

    def test_dispatch_raises_on_missing_type_url(self):
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        cmd = types.ContextualCommand(
            command=types.CommandBook(
                cover=types.Cover(domain="player"),
                pages=[types.CommandPage(command=any_pb2.Any())],  # No type_url
            ),
        )

        with pytest.raises(ValueError, match="No command pages"):
            router.dispatch(cmd)


# =============================================================================
# SagaRouter Tests
# =============================================================================


class TestSagaRouter:
    """Tests for SagaRouter (single-domain, events -> commands, stateless)."""

    def test_construction_sets_name_and_domain(self):
        handler = MockSagaHandler()
        router = SagaRouter("saga-inventory", "inventory", handler)

        assert router.name == "saga-inventory"
        assert router.input_domain == "inventory"

    def test_event_types_delegates_to_handler(self):
        handler = MockSagaHandler()
        router = SagaRouter("saga-inventory", "inventory", handler)

        assert router.event_types() == ["StockUpdated"]

    def test_subscriptions_returns_domain_and_types(self):
        handler = MockSagaHandler()
        router = SagaRouter("saga-inventory", "inventory", handler)

        subs = router.subscriptions()
        assert subs == [("inventory", ["StockUpdated"])]

    def test_prepare_destinations_returns_covers(self):
        handler = MockSagaHandler()
        router = SagaRouter("saga-inventory", "inventory", handler)

        source = make_event_book("inventory", "StockUpdated")
        destinations = router.prepare_destinations(source)

        assert len(destinations) == 1
        assert destinations[0].domain == "inventory"

    def test_prepare_destinations_returns_empty_for_none(self):
        handler = MockSagaHandler()
        router = SagaRouter("saga-inventory", "inventory", handler)

        destinations = router.prepare_destinations(None)
        assert destinations == []

    def test_dispatch_returns_commands(self):
        handler = MockSagaHandler()
        router = SagaRouter("saga-inventory", "inventory", handler)

        source = make_event_book("inventory", "StockUpdated", "corr-123")
        response = router.dispatch(source, [])

        assert len(response.commands) == 1
        assert response.commands[0].cover.domain == "inventory"

    def test_dispatch_raises_on_empty_source(self):
        handler = MockSagaHandler()
        router = SagaRouter("saga-inventory", "inventory", handler)

        source = types.EventBook(cover=types.Cover(domain="inventory"))

        with pytest.raises(ValueError, match="no events"):
            router.dispatch(source, [])


# =============================================================================
# ProcessManagerRouter Tests
# =============================================================================


class TestProcessManagerRouter:
    """Tests for ProcessManagerRouter (multi-domain, events -> commands + PM events)."""

    def test_construction_and_fluent_domain(self):
        router = (
            ProcessManagerRouter("pmg-workflow", "workflow", lambda eb: WorkflowState())
            .domain("order", MockPMOrderHandler())
            .domain("inventory", MockPMInventoryHandler())
        )

        assert router.name == "pmg-workflow"
        assert router.pm_domain == "workflow"

    def test_subscriptions_aggregates_all_domains(self):
        router = (
            ProcessManagerRouter("pmg-workflow", "workflow", lambda eb: WorkflowState())
            .domain("order", MockPMOrderHandler())
            .domain("inventory", MockPMInventoryHandler())
        )

        subs = router.subscriptions()

        # Should have both domains
        domains = {s[0] for s in subs}
        assert domains == {"order", "inventory"}

    def test_dispatch_routes_to_correct_handler(self):
        router = ProcessManagerRouter(
            "pmg-workflow", "workflow", lambda eb: WorkflowState()
        ).domain("inventory", MockPMInventoryHandler())

        trigger = make_event_book("inventory", "StockReserved", "corr-1")
        process_state = types.EventBook()

        response = router.dispatch(trigger, process_state, [])

        assert len(response.commands) == 1
        assert response.commands[0].cover.domain == "fulfillment"

    def test_dispatch_raises_on_unknown_domain(self):
        router = ProcessManagerRouter(
            "pmg-workflow", "workflow", lambda eb: WorkflowState()
        )

        trigger = make_event_book("unknown", "SomeEvent")
        process_state = types.EventBook()

        with pytest.raises(ValueError, match="No handler for domain"):
            router.dispatch(trigger, process_state, [])

    def test_prepare_destinations_works(self):
        router = ProcessManagerRouter(
            "pmg-workflow", "workflow", lambda eb: WorkflowState()
        ).domain("order", MockPMOrderHandler())

        trigger = make_event_book("order", "OrderCreated")
        process_state = types.EventBook()

        destinations = router.prepare_destinations(trigger, process_state)

        assert len(destinations) == 1
        assert destinations[0].domain == "inventory"


# =============================================================================
# ProjectorRouter Tests
# =============================================================================


class TestProjectorRouter:
    """Tests for ProjectorRouter (multi-domain, events -> external output)."""

    def test_construction_and_fluent_domain(self):
        router = ProjectorRouter("prj-inventory").domain(
            "inventory", MockProjectorHandler()
        )

        assert router.name == "prj-inventory"

    def test_subscriptions_aggregates_all_domains(self):
        router = (
            ProjectorRouter("prj-multi")
            .domain("inventory", MockProjectorHandler())
            .domain("player", MockProjectorHandler())
        )

        subs = router.subscriptions()
        domains = {s[0] for s in subs}
        assert domains == {"inventory", "player"}

    def test_dispatch_routes_to_handler(self):
        router = ProjectorRouter("prj-inventory").domain(
            "inventory", MockProjectorHandler()
        )

        events = make_event_book("inventory", "StockUpdated")
        projection = router.dispatch(events)

        assert projection.projector == "test-projector"

    def test_dispatch_raises_on_unknown_domain(self):
        router = ProjectorRouter("prj-inventory")

        events = make_event_book("unknown", "SomeEvent")

        with pytest.raises(ValueError, match="No handler for domain"):
            router.dispatch(events)


# =============================================================================
# Helper Function Tests
# =============================================================================


class TestNextSequence:
    """Tests for _next_sequence helper."""

    def test_returns_zero_for_none(self):
        assert _next_sequence(None) == 0

    def test_returns_zero_for_empty_pages(self):
        eb = types.EventBook()
        assert _next_sequence(eb) == 0

    def test_returns_page_count(self):
        eb = types.EventBook(
            pages=[
                types.EventPage(),
                types.EventPage(),
                types.EventPage(),
            ]
        )
        assert _next_sequence(eb) == 3


class TestExtractRejectionKey:
    """Tests for _extract_rejection_key helper."""

    def test_extracts_domain_and_type(self):
        rejection = types.RejectionNotification(
            rejected_command=types.CommandBook(
                cover=types.Cover(domain="inventory"),
                pages=[
                    types.CommandPage(
                        command=any_pb2.Any(
                            type_url="type.googleapis.com/test.ReserveStock"
                        )
                    )
                ],
            )
        )

        domain, cmd_type = _extract_rejection_key(rejection)

        assert domain == "inventory"
        assert cmd_type == "test.ReserveStock"

    def test_returns_empty_for_missing_command(self):
        rejection = types.RejectionNotification()

        domain, cmd_type = _extract_rejection_key(rejection)

        assert domain == ""
        assert cmd_type == ""


# =============================================================================
# Integration Tests
# =============================================================================


class TestRouterIntegration:
    """Integration tests verifying router patterns work together."""

    def test_aggregate_state_rebuilt_correctly(self):
        """Verify state is correctly rebuilt from events."""
        handler = MockPlayerHandler()
        router = AggregateRouter("player", "player", handler)

        # Create a sequence of events
        reg_event = PlayerRegistered(
            player_id="integration-test", display_name="Test User"
        )
        reg_any = any_pb2.Any()
        reg_any.Pack(reg_event)

        prior = types.EventBook(
            cover=types.Cover(domain="player"),
            pages=[types.EventPage(sequence=0, event=reg_any)],
        )

        # Rebuild state
        state = router.rebuild_state(prior)

        assert state.exists is True
        assert state.player_id == "integration-test"

    def test_saga_two_phase_protocol(self):
        """Verify saga prepare/dispatch two-phase protocol."""
        handler = MockSagaHandler()
        router = SagaRouter("saga-test", "inventory", handler)

        # Phase 1: Prepare
        source = make_event_book("inventory", "StockUpdated", "workflow-1")
        destinations = router.prepare_destinations(source)
        assert len(destinations) > 0

        # Phase 2: Dispatch with fetched destinations
        response = router.dispatch(source, [types.EventBook()])
        assert len(response.commands) > 0

    def test_pm_multi_domain_routing(self):
        """Verify PM routes to correct handler per domain."""
        router = (
            ProcessManagerRouter("pmg-test", "test-pm", lambda eb: WorkflowState())
            .domain("order", MockPMOrderHandler())
            .domain("inventory", MockPMInventoryHandler())
        )

        # Order event should route to order handler
        order_trigger = make_event_book("order", "OrderCreated", "corr-1")
        order_response = router.dispatch(order_trigger, types.EventBook(), [])
        # Order handler returns empty commands
        assert len(order_response.commands) == 0

        # Inventory event should route to inventory handler
        inv_trigger = make_event_book("inventory", "StockReserved", "corr-1")
        inv_response = router.dispatch(inv_trigger, types.EventBook(), [])
        # Inventory handler returns fulfillment command
        assert len(inv_response.commands) == 1
        assert inv_response.commands[0].cover.domain == "fulfillment"
