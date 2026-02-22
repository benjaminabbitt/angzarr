"""Compensation flow helpers for saga/PM rejection handling.

This module provides helpers for implementing compensation logic in aggregates
and process managers when saga/PM commands are rejected.

Usage in Aggregate:
    from angzarr_client import Aggregate, handles
    from angzarr_client.compensation import (
        CompensationContext,
        delegate_to_framework,
        emit_compensation_events,
    )

    class OrderAggregate(Aggregate[OrderState]):
        def handle_revocation(self, notification):
            ctx = CompensationContext.from_notification(notification)

            # Option 1: Emit compensation events
            if ctx.issuer_name == "saga-order-fulfillment":
                event = OrderCancelled(
                    order_id=self.order_id,
                    reason=f"Fulfillment failed: {ctx.rejection_reason}",
                )
                self._apply_and_record(event)
                return emit_compensation_events(self.event_book())

            # Option 2: Delegate to framework
            return delegate_to_framework(
                reason=f"No custom compensation for {ctx.issuer_name}"
            )

Usage in ProcessManager:
    from angzarr_client import ProcessManager, reacts_to
    from angzarr_client.compensation import (
        CompensationContext,
        pm_delegate_to_framework,
        pm_emit_compensation_events,
    )

    class OrderWorkflowPM(ProcessManager[WorkflowState]):
        def handle_revocation(self, notification):
            ctx = CompensationContext.from_notification(notification)

            # Record failure in PM state
            event = WorkflowStepFailed(
                issuer_name=ctx.issuer_name,
                reason=ctx.rejection_reason,
            )
            self._apply_and_record(event)

            # Return PM events + framework response
            return pm_emit_compensation_events(
                process_events=self.process_events(),
                also_emit_system_event=True,
            )
"""

from dataclasses import dataclass
from typing import Optional

from .proto.angzarr import aggregate_pb2 as aggregate
from .proto.angzarr import types_pb2 as types


@dataclass
class CompensationContext:
    """Extracted context from a rejection Notification.

    Provides easy access to compensation-relevant fields.
    """

    issuer_name: str
    """Name of the saga/PM that issued the rejected command."""

    issuer_type: str
    """Type of issuer: 'saga' or 'process_manager'."""

    source_event_sequence: int
    """Sequence of the event that triggered the saga/PM flow."""

    rejection_reason: str
    """Why the command was rejected."""

    rejected_command: Optional[types.CommandBook]
    """The command that was rejected (if available)."""

    source_aggregate: Optional[types.Cover]
    """Cover of the aggregate that triggered the flow."""

    @classmethod
    def from_notification(
        cls, notification: types.Notification
    ) -> "CompensationContext":
        """Extract compensation context from a Notification.

        Args:
            notification: The notification containing RejectionNotification payload.

        Returns:
            CompensationContext with extracted fields.
        """
        rejection = types.RejectionNotification()
        if notification.HasField("payload"):
            notification.payload.Unpack(rejection)

        return cls(
            issuer_name=rejection.issuer_name,
            issuer_type=rejection.issuer_type,
            source_event_sequence=rejection.source_event_sequence,
            rejection_reason=rejection.rejection_reason,
            rejected_command=rejection.rejected_command
            if rejection.HasField("rejected_command")
            else None,
            source_aggregate=rejection.source_aggregate
            if rejection.HasField("source_aggregate")
            else None,
        )

    @property
    def rejected_command_type(self) -> Optional[str]:
        """Get the type URL of the rejected command, if available."""
        if self.rejected_command and self.rejected_command.pages:
            page = self.rejected_command.pages[0]
            if page.HasField("command"):
                return page.command.type_url
        return None


# --- Aggregate helpers ---


def delegate_to_framework(
    reason: str,
    emit_system_event: bool = True,
    send_to_dead_letter: bool = False,
    escalate: bool = False,
    abort: bool = False,
) -> aggregate.BusinessResponse:
    """Create a response that delegates compensation to the framework.

    Use when the aggregate doesn't have custom compensation logic for a saga.
    The framework will emit a SagaCompensationFailed event to the fallback domain.

    Args:
        reason: Human-readable explanation for the delegation.
        emit_system_event: Emit SagaCompensationFailed to fallback domain.
        send_to_dead_letter: Move failed event to dead letter queue.
        escalate: Mark for operator intervention.
        abort: Stop the saga entirely without retry.

    Returns:
        BusinessResponse with revocation flags.
    """
    return aggregate.BusinessResponse(
        revocation=aggregate.RevocationResponse(
            emit_system_revocation=emit_system_event,
            send_to_dead_letter_queue=send_to_dead_letter,
            escalate=escalate,
            abort=abort,
            reason=reason,
        )
    )


def emit_compensation_events(event_book: types.EventBook) -> aggregate.BusinessResponse:
    """Create a response containing compensation events.

    Use when the aggregate emits events to record compensation.
    The framework will persist these events and NOT emit a system event.

    Args:
        event_book: EventBook containing compensation events.

    Returns:
        BusinessResponse with events.
    """
    return aggregate.BusinessResponse(events=event_book)


# --- Process Manager helpers ---


@dataclass
class RejectionHandlerResponse:
    """Response from rejection handlers.

    Can contain events (compensation), notification (upstream propagation), or both.
    """

    events: Optional[types.EventBook] = None
    """Events to persist to own state (compensation)."""

    notification: Optional[types.Notification] = None
    """Notification to forward upstream (rejection propagation)."""


def pm_delegate_to_framework(
    reason: str,
    emit_system_event: bool = True,
) -> tuple[None, aggregate.RevocationResponse]:
    """Create a PM response that delegates compensation to the framework.

    Use when the PM doesn't have custom compensation logic.

    Args:
        reason: Human-readable explanation for the delegation.
        emit_system_event: Emit SagaCompensationFailed to fallback domain.

    Returns:
        Tuple of (None, RevocationResponse) - no PM events, delegate to framework.
    """
    return None, aggregate.RevocationResponse(
        emit_system_revocation=emit_system_event,
        reason=reason,
    )


def pm_emit_compensation_events(
    process_events: types.EventBook,
    also_emit_system_event: bool = False,
    reason: str = "",
) -> tuple[types.EventBook, aggregate.RevocationResponse]:
    """Create a PM response containing compensation events.

    Use when the PM emits events to record the compensation in its state.

    Args:
        process_events: EventBook containing PM compensation events.
        also_emit_system_event: Also emit SagaCompensationFailed.
        reason: Reason for system event (if emitting).

    Returns:
        Tuple of (EventBook, RevocationResponse).
    """
    return process_events, aggregate.RevocationResponse(
        emit_system_revocation=also_emit_system_event,
        reason=reason,
    )
