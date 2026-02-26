"""Tests for @rejected decorator and rejection handling.

Tests rejection/compensation handling in both CommandHandler (aggregates)
and ProcessManager components.
"""

from dataclasses import dataclass

import pytest
from google.protobuf import any_pb2

from angzarr_client.aggregate import CommandHandler, applies, handles
from angzarr_client.process_manager import ProcessManager
from angzarr_client.process_manager import handles as pm_handles
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import rejected

from .fixtures import (
    DepositFunds,
    FundsDeposited,
    FundsReleased,
    OrderCreated,
    PlayerRegistered,
    ProcessPayment,
    RegisterPlayer,
    ReserveStock,
    WorkflowFailed,
)

# =============================================================================
# Test state classes
# =============================================================================


@dataclass
class PlayerState:
    """State for player aggregate."""

    player_id: str = ""
    display_name: str = ""
    bankroll: int = 0
    reserved_amount: int = 0


@dataclass
class WorkflowState:
    """State for workflow process manager."""

    order_id: str = ""
    failed: bool = False
    failure_reason: str = ""


# =============================================================================
# Test aggregates with @rejected handlers
# =============================================================================


class PlayerWithRejectionHandler(CommandHandler[PlayerState]):
    """Player aggregate with rejection handler for payment failures."""

    domain = "player"

    def _create_empty_state(self) -> PlayerState:
        return PlayerState()

    @applies(PlayerRegistered)
    def apply_registered(self, state: PlayerState, event: PlayerRegistered):
        state.player_id = event.player_id
        state.display_name = event.display_name

    @applies(FundsDeposited)
    def apply_deposited(self, state: PlayerState, event: FundsDeposited):
        state.bankroll = event.new_bankroll

    @applies(FundsReleased)
    def apply_released(self, state: PlayerState, event: FundsReleased):
        state.bankroll += event.amount
        state.reserved_amount = 0

    @handles(RegisterPlayer)
    def register(self, cmd: RegisterPlayer) -> PlayerRegistered:
        return PlayerRegistered(
            player_id=f"player_{cmd.email}",
            display_name=cmd.display_name,
        )

    @handles(DepositFunds)
    def deposit(self, cmd: DepositFunds) -> FundsDeposited:
        return FundsDeposited(new_bankroll=self.state.bankroll + cmd.amount)

    @rejected(domain="payment", command="ProcessPayment")
    def handle_payment_rejected(
        self, notification: types.Notification
    ) -> FundsReleased:
        """Release reserved funds when payment fails."""
        rejection = types.RejectionNotification()
        if notification.HasField("payload"):
            notification.payload.Unpack(rejection)
        return FundsReleased(
            amount=100,  # Would normally come from state
            reason=rejection.rejection_reason,
        )


class PlayerWithoutRejectionHandler(CommandHandler[PlayerState]):
    """Player aggregate without rejection handler (uses framework default)."""

    domain = "player"

    def _create_empty_state(self) -> PlayerState:
        return PlayerState()

    @applies(PlayerRegistered)
    def apply_registered(self, state: PlayerState, event: PlayerRegistered):
        state.player_id = event.player_id

    @handles(RegisterPlayer)
    def register(self, cmd: RegisterPlayer) -> PlayerRegistered:
        return PlayerRegistered(player_id=f"player_{cmd.email}")


# =============================================================================
# Test process managers with @rejected handlers
# =============================================================================


class WorkflowPMWithRejectionHandler(ProcessManager[WorkflowState]):
    """Process manager with rejection handler for inventory failures."""

    name = "workflow-pm"

    def _create_empty_state(self) -> WorkflowState:
        return WorkflowState()

    def _apply_event(self, state: WorkflowState, event_any: any_pb2.Any):
        if event_any.type_url.endswith("OrderCreated"):
            event = OrderCreated()
            event_any.Unpack(event)
            state.order_id = event.order_id
        elif event_any.type_url.endswith("WorkflowFailed"):
            event = WorkflowFailed()
            event_any.Unpack(event)
            state.failed = True
            state.failure_reason = event.reason

    @pm_handles(OrderCreated, input_domain="order", output_domain="inventory")
    def on_order_created(self, event: OrderCreated) -> ReserveStock:
        return ReserveStock(order_id=event.order_id, sku="SKU-123", quantity=1)

    @rejected(domain="inventory", command="ReserveStock")
    def handle_reserve_rejected(
        self, notification: types.Notification
    ) -> WorkflowFailed:
        """Mark workflow as failed when inventory reservation fails."""
        rejection = types.RejectionNotification()
        if notification.HasField("payload"):
            notification.payload.Unpack(rejection)
        return WorkflowFailed(
            reason=rejection.rejection_reason,
            failed_domain="inventory",
            failed_command="ReserveStock",
        )


# =============================================================================
# Helper functions
# =============================================================================


def make_rejection_notification(
    domain: str,
    command_type: type,
    reason: str = "Test rejection",
) -> types.Notification:
    """Create a rejection notification for testing."""
    # Create the rejected command
    cmd = command_type()
    cmd_any = any_pb2.Any()
    cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

    rejected_command = types.CommandBook(
        cover=types.Cover(domain=domain),
        pages=[types.CommandPage(command=cmd_any)],
    )

    # Create rejection notification
    rejection = types.RejectionNotification(
        rejection_reason=reason,
        rejected_command=rejected_command,
    )

    # Wrap in Notification
    payload = any_pb2.Any()
    payload.Pack(rejection, type_url_prefix="type.googleapis.com/")

    return types.Notification(payload=payload)


# =============================================================================
# Tests for @rejected decorator metadata
# =============================================================================


class TestRejectedDecoratorMetadata:
    """Test that @rejected sets correct metadata on methods."""

    def test_sets_is_rejection_handler(self):
        method = PlayerWithRejectionHandler.handle_payment_rejected
        assert hasattr(method, "_is_rejection_handler")
        assert method._is_rejection_handler is True

    def test_sets_rejection_domain(self):
        method = PlayerWithRejectionHandler.handle_payment_rejected
        assert hasattr(method, "_rejection_domain")
        assert method._rejection_domain == "payment"

    def test_sets_rejection_command(self):
        method = PlayerWithRejectionHandler.handle_payment_rejected
        assert hasattr(method, "_rejection_command")
        assert method._rejection_command == "ProcessPayment"

    def test_preserves_function_name(self):
        method = PlayerWithRejectionHandler.handle_payment_rejected
        assert method.__name__ == "handle_payment_rejected"


# =============================================================================
# Tests for @rejected in CommandHandler (aggregates)
# =============================================================================


class TestCommandHandlerRejection:
    """Test rejection handling in CommandHandler."""

    def test_builds_rejection_table(self):
        """Rejection table is built from @rejected methods."""
        table = PlayerWithRejectionHandler._rejection_table
        assert "payment/ProcessPayment" in table
        assert table["payment/ProcessPayment"] == "handle_payment_rejected"

    def test_handle_revocation_dispatches_to_handler(self):
        """handle_revocation dispatches to matching @rejected handler."""
        agg = PlayerWithRejectionHandler()
        notification = make_rejection_notification(
            domain="payment",
            command_type=ProcessPayment,
            reason="Insufficient funds",
        )

        response = agg.handle_revocation(notification)

        # Should return events (compensation event was emitted)
        assert response.HasField("events")
        assert len(response.events.pages) == 1

        # Verify the compensation event
        event_any = response.events.pages[0].event
        assert event_any.type_url.endswith("FundsReleased")

        event = FundsReleased()
        event_any.Unpack(event)
        assert event.amount == 100
        assert event.reason == "Insufficient funds"

    def test_handle_revocation_no_handler_delegates_to_framework(self):
        """handle_revocation returns revocation response when no handler matches."""
        agg = PlayerWithoutRejectionHandler()
        notification = make_rejection_notification(
            domain="payment",
            command_type=ProcessPayment,
            reason="Test rejection",
        )

        response = agg.handle_revocation(notification)

        # Should return revocation (delegate to framework)
        assert response.HasField("revocation")
        assert response.revocation.emit_system_revocation is True
        assert "no custom compensation" in response.revocation.reason.lower()

    def test_handle_revocation_wrong_domain_delegates_to_framework(self):
        """handle_revocation delegates to framework for unhandled domain."""
        agg = PlayerWithRejectionHandler()
        # Create notification for wrong domain
        notification = make_rejection_notification(
            domain="inventory",  # Handler is for "payment"
            command_type=ReserveStock,
            reason="Out of stock",
        )

        response = agg.handle_revocation(notification)

        # Should delegate to framework
        assert response.HasField("revocation")
        assert response.revocation.emit_system_revocation is True


class TestCommandHandlerRejectionWithState:
    """Test rejection handling preserves state correctly."""

    def test_rejection_handler_can_access_state(self):
        """@rejected handler can access aggregate state."""
        # Create aggregate with prior events
        registered_event = PlayerRegistered(player_id="p1", display_name="Alice")
        event_any = any_pb2.Any()
        event_any.Pack(registered_event, type_url_prefix="type.googleapis.com/")

        event_book = types.EventBook(
            pages=[types.EventPage(event=event_any)],
        )

        agg = PlayerWithRejectionHandler(event_book)

        # Verify state is accessible
        assert agg.state.player_id == "p1"
        assert agg.state.display_name == "Alice"

        # Handle rejection
        notification = make_rejection_notification(
            domain="payment",
            command_type=ProcessPayment,
            reason="Card declined",
        )

        response = agg.handle_revocation(notification)

        # Compensation event should be emitted
        assert response.HasField("events")


# =============================================================================
# Tests for @rejected in ProcessManager
# =============================================================================


class TestProcessManagerRejection:
    """Test rejection handling in ProcessManager."""

    def test_builds_rejection_table(self):
        """Rejection table is built from @rejected methods."""
        table = WorkflowPMWithRejectionHandler._rejection_table
        assert "inventory/ReserveStock" in table
        assert table["inventory/ReserveStock"] == "handle_reserve_rejected"

    def test_handle_revocation_dispatches_to_handler(self):
        """handle_revocation dispatches to matching @rejected handler."""
        pm = WorkflowPMWithRejectionHandler()
        notification = make_rejection_notification(
            domain="inventory",
            command_type=ReserveStock,
            reason="Out of stock",
        )

        response = pm.handle_revocation(notification)

        # Should return events (compensation event was recorded)
        assert response.events is not None
        assert len(response.events.pages) == 1

        # Verify the compensation event
        event_any = response.events.pages[0].event
        assert event_any.type_url.endswith("WorkflowFailed")

        event = WorkflowFailed()
        event_any.Unpack(event)
        assert event.reason == "Out of stock"
        assert event.failed_domain == "inventory"
        assert event.failed_command == "ReserveStock"


# =============================================================================
# Tests for duplicate @rejected handlers
# =============================================================================


class TestRejectedDuplicateHandlers:
    """Test that duplicate @rejected handlers raise errors."""

    def test_duplicate_rejection_handler_raises(self):
        """Duplicate @rejected for same domain/command raises TypeError."""
        with pytest.raises(TypeError, match="duplicate rejection handler"):

            class BadAggregate(CommandHandler[PlayerState]):
                domain = "player"

                def _create_empty_state(self) -> PlayerState:
                    return PlayerState()

                @rejected(domain="payment", command="ProcessPayment")
                def handle_one(self, notification):
                    pass

                @rejected(domain="payment", command="ProcessPayment")
                def handle_two(self, notification):
                    pass
