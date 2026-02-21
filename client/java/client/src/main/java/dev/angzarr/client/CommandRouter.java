package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;

import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.util.*;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * DRY command dispatcher for aggregates (functional pattern).
 */
public class CommandRouter<S> {
    private final String domain;
    private Function<EventBook, S> rebuild;
    private StateRouter<S> stateRouter;
    private final List<Map.Entry<String, CommandHandler<S>>> handlers = new ArrayList<>();
    private final Map<String, RejectionHandler<S>> rejectionHandlers = new HashMap<>();

    @FunctionalInterface
    public interface CommandHandler<S> {
        EventBook handle(CommandBook commandBook, Any commandAny, S state, int seq);
    }

    @FunctionalInterface
    public interface RejectionHandler<S> {
        RejectionHandlerResponse handle(Notification notification, S state);
    }

    /**
     * Typed command handler - receives unpacked command and state, returns event.
     */
    @FunctionalInterface
    public interface TypedCommandHandler<C extends Message, S, E extends Message> {
        E handle(C command, S state);
    }

    public CommandRouter(String domain) {
        this.domain = domain;
    }

    public CommandRouter(String domain, Function<EventBook, S> rebuild) {
        this.domain = domain;
        this.rebuild = rebuild;
    }

    public CommandRouter<S> withState(StateRouter<S> stateRouter) {
        this.stateRouter = stateRouter;
        return this;
    }

    public CommandRouter<S> on(String suffix, CommandHandler<S> handler) {
        handlers.add(new AbstractMap.SimpleEntry<>(suffix, handler));
        return this;
    }

    /**
     * Register a typed handler for a command suffix.
     * The handler receives the unpacked command and state, returns an event.
     * The router automatically unpacks the command and wraps the event in an EventBook.
     */
    @SuppressWarnings("unchecked")
    public <C extends Message, E extends Message> CommandRouter<S> on(
            String suffix,
            Class<C> commandType,
            BiFunction<C, S, E> handler) {
        handlers.add(new AbstractMap.SimpleEntry<>(suffix, (commandBook, commandAny, state, seq) -> {
            try {
                C cmd = (C) commandAny.unpack(commandType);
                E event = handler.apply(cmd, state);
                // Wrap event in EventBook
                return EventBook.newBuilder()
                    .setCover(Cover.newBuilder()
                        .setDomain(domain))
                    .addPages(EventPage.newBuilder()
                        .setSequence(seq)
                        .setEvent(Any.pack(event, "type.googleapis.com/")))
                    .build();
            } catch (InvalidProtocolBufferException e) {
                throw new Errors.ClientError("Failed to unpack command", e);
            }
        }));
        return this;
    }

    public CommandRouter<S> onRejected(String domain, String command, RejectionHandler<S> handler) {
        rejectionHandlers.put(domain + "/" + command, handler);
        return this;
    }

    public BusinessResponse dispatch(ContextualCommand cmd) {
        var commandBook = cmd.getCommand();
        var priorEvents = cmd.hasEvents() ? cmd.getEvents() : null;

        var state = getState(priorEvents);
        var seq = Helpers.nextSequence(priorEvents);

        if (commandBook.getPagesList().isEmpty()) {
            throw new Errors.InvalidArgumentError("No command pages");
        }

        var commandAny = commandBook.getPages(0).getCommand();
        if (commandAny.getTypeUrl().isEmpty()) {
            throw new Errors.InvalidArgumentError("No command pages");
        }

        var typeUrl = commandAny.getTypeUrl();

        // Check for Notification
        if (typeUrl.endsWith("Notification")) {
            try {
                var notification = commandAny.unpack(Notification.class);
                return dispatchRejection(notification, state);
            } catch (InvalidProtocolBufferException e) {
                throw new Errors.ClientError("Failed to unpack notification", e);
            }
        }

        // Normal command dispatch
        for (var entry : handlers) {
            if (typeUrl.endsWith(entry.getKey())) {
                var events = entry.getValue().handle(commandBook, commandAny, state, seq);
                return BusinessResponse.newBuilder().setEvents(events).build();
            }
        }

        throw new Errors.InvalidArgumentError("Unknown command type: " + typeUrl);
    }

    private BusinessResponse dispatchRejection(Notification notification, S state) {
        String domain = "";
        String commandSuffix = "";

        if (notification.hasPayload()) {
            try {
                var rejection = notification.getPayload().unpack(RejectionNotification.class);
                if (rejection.hasRejectedCommand() && !rejection.getRejectedCommand().getPagesList().isEmpty()) {
                    var rejectedCmd = rejection.getRejectedCommand();
                    domain = rejectedCmd.hasCover() ? rejectedCmd.getCover().getDomain() : "";
                    var cmdTypeUrl = rejectedCmd.getPages(0).getCommand().getTypeUrl();
                    commandSuffix = Helpers.typeNameFromUrl(cmdTypeUrl);
                }
            } catch (InvalidProtocolBufferException ignored) {}
        }

        for (var entry : rejectionHandlers.entrySet()) {
            var parts = entry.getKey().split("/");
            if (parts[0].equals(domain) && commandSuffix.endsWith(parts[1])) {
                var response = entry.getValue().handle(notification, state);
                // Handle notification forwarding
                if (response.hasNotification()) {
                    return BusinessResponse.newBuilder()
                        .setNotification(response.getNotification())
                        .build();
                }
                // Handle compensation events
                if (response.hasEvents()) {
                    return BusinessResponse.newBuilder()
                        .setEvents(response.getEvents())
                        .build();
                }
                // Handler returned empty response
                return BusinessResponse.newBuilder()
                    .setRevocation(RevocationResponse.newBuilder()
                        .setEmitSystemRevocation(false)
                        .setReason("Aggregate " + this.domain + " handled rejection for " + entry.getKey())
                        .build())
                    .build();
            }
        }

        return BusinessResponse.newBuilder()
            .setRevocation(RevocationResponse.newBuilder()
                .setEmitSystemRevocation(true)
                .setReason("Aggregate " + this.domain + " has no custom compensation for " + domain + "/" + commandSuffix)
                .build())
            .build();
    }

    private S getState(EventBook eventBook) {
        if (stateRouter != null) {
            return stateRouter.withEventBook(eventBook);
        }
        if (rebuild != null) {
            return rebuild.apply(eventBook);
        }
        throw new IllegalStateException("CommandRouter requires either rebuild function or StateRouter");
    }
}
