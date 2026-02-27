package dev.angzarr.client.compensation;

import dev.angzarr.BusinessResponse;
import dev.angzarr.EventBook;
import dev.angzarr.RevocationResponse;

/**
 * Static helper methods for compensation flow handling.
 *
 * <p>Provides convenient factory methods for creating compensation responses
 * in aggregates and process managers when handling saga/PM rejections.
 *
 * <p>Usage in Aggregate:
 * <pre>{@code
 * @Rejected(domain = "payment", command = "ProcessPayment")
 * public BusinessResponse handlePaymentRejected(Notification notification) {
 *     var ctx = CompensationContext.from(notification);
 *
 *     // Option 1: Emit compensation events
 *     if (ctx.getIssuerName().equals("saga-order-fulfillment")) {
 *         var event = OrderCancelled.newBuilder()
 *             .setOrderId(getOrderId())
 *             .setReason("Fulfillment failed: " + ctx.getRejectionReason())
 *             .build();
 *         apply(event);
 *         return Compensation.emitCompensationEvents(getEventBook());
 *     }
 *
 *     // Option 2: Delegate to framework
 *     return Compensation.delegateToFramework(
 *         "No custom compensation for " + ctx.getIssuerName()
 *     );
 * }
 * }</pre>
 */
public final class Compensation {

    private Compensation() {
        // Utility class
    }

    // --- Aggregate helpers ---

    /**
     * Create a response that delegates compensation to the framework.
     *
     * <p>Use when the aggregate doesn't have custom compensation logic for a saga.
     * The framework will emit a SagaCompensationFailed event to the fallback domain.
     *
     * @param reason Human-readable explanation for the delegation.
     * @return BusinessResponse with revocation flags.
     */
    public static BusinessResponse delegateToFramework(String reason) {
        return delegateToFramework(reason, true, false, false, false);
    }

    /**
     * Create a response that delegates compensation to the framework with options.
     *
     * @param reason Human-readable explanation for the delegation.
     * @param emitSystemEvent Emit SagaCompensationFailed to fallback domain.
     * @param sendToDeadLetter Move failed event to dead letter queue.
     * @param escalate Mark for operator intervention.
     * @param abort Stop the saga entirely without retry.
     * @return BusinessResponse with revocation flags.
     */
    public static BusinessResponse delegateToFramework(
            String reason,
            boolean emitSystemEvent,
            boolean sendToDeadLetter,
            boolean escalate,
            boolean abort) {
        return BusinessResponse.newBuilder()
            .setRevocation(RevocationResponse.newBuilder()
                .setEmitSystemRevocation(emitSystemEvent)
                .setSendToDeadLetterQueue(sendToDeadLetter)
                .setEscalate(escalate)
                .setAbort(abort)
                .setReason(reason)
                .build())
            .build();
    }

    /**
     * Create a response containing compensation events.
     *
     * <p>Use when the aggregate emits events to record compensation.
     * The framework will persist these events and NOT emit a system event.
     *
     * @param eventBook EventBook containing compensation events.
     * @return BusinessResponse with events.
     */
    public static BusinessResponse emitCompensationEvents(EventBook eventBook) {
        return BusinessResponse.newBuilder()
            .setEvents(eventBook)
            .build();
    }

    // --- Process Manager helpers ---

    /**
     * Create a PM response that delegates compensation to the framework.
     *
     * <p>Use when the PM doesn't have custom compensation logic.
     *
     * @param reason Human-readable explanation for the delegation.
     * @return RevocationResponse - no PM events, delegate to framework.
     */
    public static RevocationResponse pmDelegateToFramework(String reason) {
        return pmDelegateToFramework(reason, true);
    }

    /**
     * Create a PM response that delegates compensation to the framework.
     *
     * @param reason Human-readable explanation for the delegation.
     * @param emitSystemEvent Emit SagaCompensationFailed to fallback domain.
     * @return RevocationResponse - no PM events, delegate to framework.
     */
    public static RevocationResponse pmDelegateToFramework(String reason, boolean emitSystemEvent) {
        return RevocationResponse.newBuilder()
            .setEmitSystemRevocation(emitSystemEvent)
            .setReason(reason)
            .build();
    }

    /**
     * Create a PM response containing compensation events.
     *
     * <p>Use when the PM emits events to record the compensation in its state.
     *
     * @param processEvents EventBook containing PM compensation events.
     * @return RejectionHandlerResponse with events.
     */
    public static RejectionHandlerResponse pmEmitCompensationEvents(EventBook processEvents) {
        return RejectionHandlerResponse.withEvents(processEvents);
    }

    /**
     * Create a PM response with events and system event emission.
     *
     * @param processEvents EventBook containing PM compensation events.
     * @param reason Reason for system event.
     * @return RejectionHandlerResponse with events.
     */
    public static RejectionHandlerResponse pmEmitCompensationEventsWithSystemEvent(
            EventBook processEvents, String reason) {
        // The RejectionHandlerResponse will be combined with RevocationResponse by the router
        return RejectionHandlerResponse.withEvents(processEvents);
    }
}
