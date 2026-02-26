package dev.angzarr.client.router;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import dev.angzarr.*;
import dev.angzarr.client.Helpers;
import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.util.AbstractMap;
import java.util.List;
import java.util.Map;

/**
 * Router for command handler components (commands -> events, single domain).
 *
 * <p>Domain is set at construction time. No additional domain registration
 * is possible, enforcing single-domain constraint.
 *
 * <p>Example:
 * <pre>{@code
 * CommandHandlerRouter<PlayerState> router = new CommandHandlerRouter<>(
 *     "player",           // router name
 *     "player",           // domain
 *     new PlayerHandler() // handler
 * );
 *
 * // Get subscriptions for registration
 * List<Map.Entry<String, List<String>>> subs = router.subscriptions();
 *
 * // Dispatch a command
 * BusinessResponse response = router.dispatch(contextualCommand);
 * }</pre>
 *
 * @param <S> The state type for this command handler
 */
public class CommandHandlerRouter<S> {

    private final String name;
    private final String domain;
    private final CommandHandlerDomainHandler<S> handler;

    /**
     * Create a new command handler router.
     *
     * @param name The router name
     * @param domain The domain this command handler handles
     * @param handler The domain handler
     */
    public CommandHandlerRouter(String name, String domain, CommandHandlerDomainHandler<S> handler) {
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
     * Get the domain.
     */
    public String getDomain() {
        return domain;
    }

    /**
     * Get command types from the handler.
     */
    public List<String> getCommandTypes() {
        return handler.commandTypes();
    }

    /**
     * Get subscriptions for this command handler.
     *
     * @return List of (domain, command types) pairs
     */
    public List<Map.Entry<String, List<String>>> subscriptions() {
        return List.of(new AbstractMap.SimpleEntry<>(domain, handler.commandTypes()));
    }

    /**
     * Rebuild state from events using the handler's state router.
     *
     * @param events The event book containing prior events
     * @return The rebuilt state
     */
    public S rebuildState(EventBook events) {
        return handler.rebuild(events);
    }

    /**
     * Dispatch a contextual command to the handler.
     *
     * @param cmd The contextual command containing command and prior events
     * @return The business response containing resulting events or rejection
     * @throws RouterException if dispatch fails
     */
    public BusinessResponse dispatch(ContextualCommand cmd) throws RouterException {
        // Validate command structure
        CommandBook commandBook = cmd.getCommand();
        if (commandBook == null || commandBook.getPagesList().isEmpty()) {
            throw new RouterException("Missing command book or pages");
        }

        CommandPage commandPage = commandBook.getPages(0);
        if (!commandPage.hasCommand()) {
            throw new RouterException("Missing command payload");
        }

        Any commandAny = commandPage.getCommand();
        EventBook eventBook = cmd.hasEvents() ? cmd.getEvents() : EventBook.getDefaultInstance();

        // Rebuild state
        S state = handler.rebuild(eventBook);
        int seq = Helpers.nextSequence(eventBook);

        String typeUrl = commandAny.getTypeUrl();

        // Check for Notification (rejection/compensation)
        if (typeUrl.endsWith("Notification")) {
            return dispatchNotification(commandAny, state);
        }

        // Execute handler
        try {
            EventBook resultBook = handler.handle(commandBook, commandAny, state, seq);
            return BusinessResponse.newBuilder()
                    .setEvents(resultBook)
                    .build();
        } catch (CommandRejectedError e) {
            throw new RouterException("Command rejected: " + e.getReason(), e);
        }
    }

    /**
     * Dispatch a notification to the rejection handler.
     */
    private BusinessResponse dispatchNotification(Any commandAny, S state) throws RouterException {
        try {
            Notification notification = commandAny.unpack(Notification.class);

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
                    notification, state, targetDomain, targetCommand);

            if (response.hasEvents()) {
                return BusinessResponse.newBuilder()
                        .setEvents(response.getEvents())
                        .build();
            } else if (response.hasNotification()) {
                return BusinessResponse.newBuilder()
                        .setNotification(response.getNotification())
                        .build();
            } else {
                return BusinessResponse.newBuilder()
                        .setRevocation(RevocationResponse.newBuilder()
                                .setEmitSystemRevocation(true)
                                .setSendToDeadLetterQueue(false)
                                .setEscalate(false)
                                .setAbort(false)
                                .setReason(String.format(
                                        "Handler returned empty response for %s/%s",
                                        targetDomain, targetCommand))
                                .build())
                        .build();
            }
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
