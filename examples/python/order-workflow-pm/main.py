"""Order Workflow Process Manager (OO Pattern)

Demonstrates the OO-style ProcessManager using:
- @prepares decorator for destination declaration
- @reacts_to decorator for event handling
- @rejected decorator for compensation

This PM coordinates an order workflow across order, inventory, and payment domains.
"""

import sys
from dataclasses import dataclass
from pathlib import Path

import structlog
from google.protobuf.any_pb2 import Any as ProtoAny

sys.path.insert(0, str(Path(__file__).parent.parent))
sys.path.insert(0, str(Path(__file__).parent.parent.parent / "client" / "python"))

from angzarr_client import ProcessManager, prepares, reacts_to, rejected
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.process_manager_handler import (
    ProcessManagerHandler,
    run_process_manager_server,
)

structlog.configure(
    processors=[
        structlog.stdlib.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.make_filtering_bound_logger(0),
    context_class=dict,
    logger_factory=structlog.PrintLoggerFactory(),
)

logger = structlog.get_logger()


# =============================================================================
# Stub proto messages (would come from generated proto in real implementation)
# =============================================================================


@dataclass
class OrderCreated:
    """Event from order domain."""

    order_id: str = ""
    customer_id: str = ""
    amount: int = 0


@dataclass
class InventoryReserved:
    """Event from inventory domain."""

    order_id: str = ""
    sku: str = ""


@dataclass
class PaymentReceived:
    """Event from payment domain."""

    order_id: str = ""
    amount: int = 0


@dataclass
class ReserveInventory:
    """Command to inventory domain."""

    order_id: str = ""
    sku: str = ""


@dataclass
class ProcessPayment:
    """Command to payment domain."""

    order_id: str = ""
    amount: int = 0


@dataclass
class WorkflowCompleted:
    """Process manager internal event."""

    order_id: str = ""


@dataclass
class WorkflowFailed:
    """Process manager internal event for failures."""

    order_id: str = ""
    reason: str = ""


# =============================================================================
# Process Manager State
# =============================================================================


@dataclass
class OrderWorkflowState:
    """State for order workflow process manager."""

    order_id: str = ""
    customer_id: str = ""
    amount: int = 0
    inventory_reserved: bool = False
    payment_received: bool = False
    failed: bool = False
    failure_reason: str = ""


# =============================================================================
# Process Manager Implementation
# =============================================================================


class OrderWorkflowPM(ProcessManager[OrderWorkflowState]):
    """Order workflow process manager using OO pattern.

    Coordinates:
    1. OrderCreated (order domain) → ReserveInventory (inventory domain)
    2. InventoryReserved (inventory domain) → ProcessPayment (payment domain)
    3. PaymentReceived (payment domain) → workflow complete

    Handles rejections:
    - ReserveInventory rejected → WorkflowFailed event
    - ProcessPayment rejected → (would need to release inventory)
    """

    name = "pmg-order-workflow"

    def _create_empty_state(self) -> OrderWorkflowState:
        return OrderWorkflowState()

    def _apply_event(self, state: OrderWorkflowState, event_any: ProtoAny) -> None:
        """Apply process manager events to state."""
        type_url = event_any.type_url
        if type_url.endswith("WorkflowCompleted"):
            # Mark as complete
            pass
        elif type_url.endswith("WorkflowFailed"):
            state.failed = True

    # -------------------------------------------------------------------------
    # Prepare handlers - declare destinations needed
    # -------------------------------------------------------------------------

    @prepares(OrderCreated)
    def prepare_order_created(self, event: OrderCreated) -> list[types.Cover]:
        """Declare inventory aggregate as destination."""
        return [
            types.Cover(
                domain="inventory",
                # In real impl, would compute root from event data
            )
        ]

    @prepares(InventoryReserved)
    def prepare_inventory_reserved(self, event: InventoryReserved) -> list[types.Cover]:
        """Declare payment aggregate as destination."""
        return [
            types.Cover(
                domain="payment",
            )
        ]

    # -------------------------------------------------------------------------
    # Event handlers
    # -------------------------------------------------------------------------

    @reacts_to(OrderCreated, input_domain="order")
    def on_order_created(
        self,
        event: OrderCreated,
        destinations: list[types.EventBook],
    ) -> ReserveInventory:
        """React to OrderCreated by issuing ReserveInventory."""
        # Update local state
        self.state.order_id = event.order_id
        self.state.customer_id = event.customer_id
        self.state.amount = event.amount

        return ReserveInventory(
            order_id=event.order_id,
            sku="default-sku",  # Would come from order details
        )

    @reacts_to(InventoryReserved, input_domain="inventory")
    def on_inventory_reserved(
        self,
        event: InventoryReserved,
        destinations: list[types.EventBook],
    ) -> ProcessPayment:
        """React to InventoryReserved by issuing ProcessPayment."""
        self.state.inventory_reserved = True

        return ProcessPayment(
            order_id=event.order_id,
            amount=self.state.amount,
        )

    @reacts_to(PaymentReceived, input_domain="payment")
    def on_payment_received(self, event: PaymentReceived) -> None:
        """React to PaymentReceived by marking workflow complete."""
        self.state.payment_received = True

        # Record completion in PM state
        self._apply_and_record(WorkflowCompleted(order_id=event.order_id))

        # No command to emit - workflow is done
        return None

    # -------------------------------------------------------------------------
    # Rejection handlers
    # -------------------------------------------------------------------------

    @rejected(domain="inventory", command="ReserveInventory")
    def handle_inventory_rejection(self, notification: types.Notification) -> None:
        """Handle when ReserveInventory is rejected."""
        # Record failure in PM state
        self._apply_and_record(
            WorkflowFailed(
                order_id=self.state.order_id,
                reason="Inventory reservation failed",
            )
        )

    @rejected(domain="payment", command="ProcessPayment")
    def handle_payment_rejection(self, notification: types.Notification) -> None:
        """Handle when ProcessPayment is rejected.

        In a real implementation, this would also need to release the
        inventory reservation that was already made.
        """
        self._apply_and_record(
            WorkflowFailed(
                order_id=self.state.order_id,
                reason="Payment processing failed",
            )
        )


if __name__ == "__main__":
    handler = ProcessManagerHandler(OrderWorkflowPM)
    run_process_manager_server(handler, "50420", logger=logger)
