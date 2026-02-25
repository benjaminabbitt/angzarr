package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.CommandBook;
import dev.angzarr.Cover;
import dev.angzarr.EventBook;

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
     * Execute phase - produce commands.
     *
     * <p>Called with source event and fetched destination state.
     * Returns commands to send to other aggregates.
     *
     * @param source The source event book
     * @param event The triggering event as an Any
     * @param destinations The fetched destination event books
     * @return List of commands to send
     * @throws CommandRejectedError if the event cannot be processed
     */
    List<CommandBook> execute(EventBook source, Any event, List<EventBook> destinations)
            throws CommandRejectedError;
}
