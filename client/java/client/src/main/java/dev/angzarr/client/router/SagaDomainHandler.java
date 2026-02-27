package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.CommandBook;
import dev.angzarr.Cover;
import dev.angzarr.EventBook;
import dev.angzarr.Notification;
import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.util.List;

/**
 * Handler interface for a single domain's events in a saga.
 *
 * <p>Sagas translate events from one domain into commands for another.
 * They are stateless - each event is processed independently.
 *
 * <p>Example:
 * <pre>{@code
 * public class OrderSagaHandler implements SagaDomainHandler {
 *
 *     @Override
 *     public List<String> eventTypes() {
 *         return List.of("OrderCompleted", "OrderCancelled");
 *     }
 *
 *     @Override
 *     public List<Cover> prepare(EventBook source, Any event) {
 *         String typeUrl = event.getTypeUrl();
 *         if (typeUrl.endsWith("OrderCompleted")) {
 *             return prepareCompleted(source, event);
 *         } else if (typeUrl.endsWith("OrderCancelled")) {
 *             return prepareCancelled(source, event);
 *         }
 *         return List.of();
 *     }
 *
 *     @Override
 *     public List<CommandBook> execute(EventBook source, Any event, List<EventBook> destinations)
 *             throws CommandRejectedError {
 *         String typeUrl = event.getTypeUrl();
 *         if (typeUrl.endsWith("OrderCompleted")) {
 *             return handleCompleted(source, event, destinations);
 *         } else if (typeUrl.endsWith("OrderCancelled")) {
 *             return handleCancelled(source, event, destinations);
 *         }
 *         return List.of();
 *     }
 * }
 * }</pre>
 */
public interface SagaDomainHandler {

    /**
     * Event type suffixes this handler processes.
     *
     * <p>Used for subscription derivation.
     *
     * @return List of event type suffixes (e.g., "OrderCompleted", "OrderCancelled")
     */
    List<String> eventTypes();

    /**
     * Prepare phase - declare destination covers needed.
     *
     * <p>Called before execute to fetch destination aggregate state.
     *
     * @param source The source event book
     * @param event The triggering event as an Any
     * @return List of covers for destinations that need to be fetched
     */
    List<Cover> prepare(EventBook source, Any event);

    /**
     * Execute phase - produce commands and/or events.
     *
     * <p>Called with source event and fetched destination state.
     * Returns commands to send to other aggregates and events to inject.
     *
     * @param source The source event book
     * @param event The triggering event as an Any
     * @param destinations The fetched destination event books
     * @return Response containing commands and events
     * @throws CommandRejectedError if the event cannot be processed
     */
    SagaHandlerResponse execute(EventBook source, Any event, List<EventBook> destinations)
            throws CommandRejectedError;

    /**
     * Handle a rejection notification.
     *
     * <p>Called when a saga-issued command was rejected. Override to provide
     * custom compensation logic.
     *
     * <p>Default implementation returns an empty response.
     *
     * @param notification The rejection notification
     * @param targetDomain The domain the rejected command targeted
     * @param targetCommand The rejected command type suffix
     * @return The rejection handler response
     * @throws CommandRejectedError if the rejection cannot be handled
     */
    default RejectionHandlerResponse onRejected(
            Notification notification,
            String targetDomain,
            String targetCommand) throws CommandRejectedError {
        return RejectionHandlerResponse.empty();
    }
}
