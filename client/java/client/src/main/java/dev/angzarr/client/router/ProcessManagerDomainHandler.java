package dev.angzarr.client.router;

import com.google.protobuf.Any;
import dev.angzarr.Cover;
import dev.angzarr.EventBook;
import dev.angzarr.Notification;
import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.util.List;

/**
 * Handler interface for a single domain's events in a process manager.
 *
 * <p>Process managers correlate events across multiple domains and maintain
 * their own state. Each domain gets its own handler, but they all share
 * the same PM state type.
 *
 * <p>Example:
 * <pre>{@code
 * public class OrderPmHandler implements ProcessManagerDomainHandler<HandFlowState> {
 *
 *     @Override
 *     public List<String> eventTypes() {
 *         return List.of("OrderCreated");
 *     }
 *
 *     @Override
 *     public List<Cover> prepare(EventBook trigger, HandFlowState state, Any event) {
 *         // Declare needed destinations
 *         return List.of();
 *     }
 *
 *     @Override
 *     public ProcessManagerResponse handle(
 *             EventBook trigger,
 *             HandFlowState state,
 *             Any event,
 *             List<EventBook> destinations) throws CommandRejectedError {
 *         // Process event, emit commands and/or PM events
 *         return ProcessManagerResponse.empty();
 *     }
 * }
 * }</pre>
 *
 * @param <S> The shared PM state type
 */
public interface ProcessManagerDomainHandler<S> {

    /**
     * Event type suffixes this handler processes.
     *
     * @return List of event type suffixes (e.g., "OrderCreated")
     */
    List<String> eventTypes();

    /**
     * Prepare phase - declare destination covers needed.
     *
     * @param trigger The triggering event book
     * @param state The current PM state
     * @param event The triggering event as an Any
     * @return List of covers for destinations that need to be fetched
     */
    List<Cover> prepare(EventBook trigger, S state, Any event);

    /**
     * Handle phase - produce commands and PM events.
     *
     * @param trigger The triggering event book
     * @param state The current PM state
     * @param event The triggering event as an Any
     * @param destinations The fetched destination event books
     * @return Response containing commands and/or process events
     * @throws CommandRejectedError if the event cannot be processed
     */
    ProcessManagerResponse handle(
            EventBook trigger,
            S state,
            Any event,
            List<EventBook> destinations) throws CommandRejectedError;

    /**
     * Handle a rejection notification.
     *
     * <p>Called when a PM-issued command was rejected. Override to provide
     * custom compensation logic.
     *
     * <p>Default implementation returns an empty response.
     *
     * @param notification The rejection notification
     * @param state The current PM state
     * @param targetDomain The domain the rejected command targeted
     * @param targetCommand The rejected command type suffix
     * @return The rejection handler response
     * @throws CommandRejectedError if the rejection cannot be handled
     */
    default RejectionHandlerResponse onRejected(
            Notification notification,
            S state,
            String targetDomain,
            String targetCommand) throws CommandRejectedError {
        return RejectionHandlerResponse.empty();
    }
}
