package dev.angzarr.client.router;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.*;
import dev.angzarr.client.Helpers;
import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.util.AbstractMap;
import java.util.Collections;
import java.util.List;
import java.util.Map;

/**
 * Router for saga components (events -> commands, single domain, stateless).
 *
 * <p>Domain is set at construction time. No additional domain registration
 * is possible, enforcing single-domain constraint.
 *
 * <p>Example:
 * <pre>{@code
 * SagaRouter router = new SagaRouter(
 *     "saga-order-fulfillment",  // router name
 *     "order",                    // input domain
 *     new OrderSagaHandler()      // handler
 * );
 *
 * // Get subscriptions for registration
 * List<Map.Entry<String, List<String>>> subs = router.subscriptions();
 *
 * // Prepare phase: get destinations needed
 * List<Cover> destinations = router.prepareDestinations(sourceEvents);
 *
 * // Execute phase: produce commands
 * SagaResponse response = router.dispatch(sourceEvents, destinationBooks);
 * }</pre>
 */
public class SagaRouter {

    private final String name;
    private final String domain;
    private final SagaDomainHandler handler;

    /**
     * Create a new saga router.
     *
     * @param name The router name
     * @param domain The input domain this saga listens to
     * @param handler The domain handler
     */
    public SagaRouter(String name, String domain, SagaDomainHandler handler) {
        this.name = name;
        this.domain = domain;
        this.handler = handler;
    }

    /**
     * Get the router name.
     */
    public String getName() {
        return name;
    }

    /**
     * Get the input domain.
     */
    public String getInputDomain() {
        return domain;
    }

    /**
     * Get event types from the handler.
     */
    public List<String> getEventTypes() {
        return handler.eventTypes();
    }

    /**
     * Get subscriptions for this saga.
     *
     * @return List of (domain, event types) pairs
     */
    public List<Map.Entry<String, List<String>>> subscriptions() {
        return List.of(new AbstractMap.SimpleEntry<>(domain, handler.eventTypes()));
    }

    /**
     * Get destinations needed for the given source events.
     *
     * <p>This is the prepare phase of the two-phase protocol.
     *
     * @param source The source event book (may be null)
     * @return List of covers for destinations that need to be fetched
     */
    public List<Cover> prepareDestinations(EventBook source) {
        if (source == null || source.getPagesList().isEmpty()) {
            return Collections.emptyList();
        }

        // Get the last event page
        EventPage eventPage = source.getPages(source.getPagesCount() - 1);
        if (!eventPage.hasEvent()) {
            return Collections.emptyList();
        }

        Any eventAny = eventPage.getEvent();
        return handler.prepare(source, eventAny);
    }

    /**
     * Dispatch an event to the saga handler.
     *
     * <p>This is the execute phase of the two-phase protocol.
     *
     * @param source The source event book
     * @param destinations The fetched destination event books
     * @return The saga response containing commands and events
     * @throws RouterException if dispatch fails
     */
    public SagaResponse dispatch(EventBook source, List<EventBook> destinations)
            throws RouterException {
        if (source == null || source.getPagesList().isEmpty()) {
            throw new RouterException("Source event book has no events");
        }

        // Get the last event page
        EventPage eventPage = source.getPages(source.getPagesCount() - 1);
        if (!eventPage.hasEvent()) {
            throw new RouterException("Missing event payload");
        }

        Any eventAny = eventPage.getEvent();

        // Check for Notification
        if (eventAny.getTypeUrl().endsWith("Notification")) {
            return dispatchNotification(eventAny);
        }

        try {
            SagaHandlerResponse response = handler.execute(source, eventAny, destinations);

            return SagaResponse.newBuilder()
                    .addAllCommands(response.getCommands())
                    .addAllEvents(response.getEvents())
                    .build();
        } catch (CommandRejectedError e) {
            throw new RouterException("Event processing failed: " + e.getReason(), e);
        }
    }

    /**
     * Dispatch a notification to the saga's rejection handler.
     */
    private SagaResponse dispatchNotification(Any eventAny) throws RouterException {
        try {
            Notification notification = eventAny.unpack(Notification.class);

            String targetDomain = "";
            String targetCommand = "";

            if (notification.hasPayload()) {
                try {
                    RejectionNotification rejection = notification.getPayload()
                            .unpack(RejectionNotification.class);
                    if (rejection.hasRejectedCommand() &&
                            rejection.getRejectedCommand().getPagesCount() > 0) {
                        CommandBook rejectedCmd = rejection.getRejectedCommand();
                        targetDomain = rejectedCmd.hasCover() ?
                                rejectedCmd.getCover().getDomain() : "";
                        targetCommand = Helpers.typeNameFromUrl(
                                rejectedCmd.getPages(0).getCommand().getTypeUrl());
                    }
                } catch (InvalidProtocolBufferException ignored) {
                    // Malformed rejection notification
                }
            }

            RejectionHandlerResponse response = handler.onRejected(
                    notification, targetDomain, targetCommand);

            SagaResponse.Builder builder = SagaResponse.newBuilder();
            if (response.hasEvents()) {
                builder.addEvents(response.getEvents());
            }
            return builder.build();
        } catch (InvalidProtocolBufferException e) {
            throw new RouterException("Failed to decode Notification", e);
        } catch (CommandRejectedError e) {
            throw new RouterException("Rejection handler failed: " + e.getReason(), e);
        }
    }

    /**
     * Exception type for router errors.
     */
    public static class RouterException extends Exception {
        public RouterException(String message) {
            super(message);
        }

        public RouterException(String message, Throwable cause) {
            super(message, cause);
        }
    }
}
