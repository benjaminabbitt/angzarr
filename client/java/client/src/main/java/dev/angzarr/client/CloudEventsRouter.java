package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.Message;
import dev.angzarr.*;

import java.util.*;
import java.util.function.Function;

/**
 * Functional router for CloudEvents projectors.
 *
 * <p>Usage:
 * <pre>{@code
 * static CloudEventsRouter buildRouter() {
 *     return new CloudEventsRouter("prj-player-cloudevents", "player")
 *         .on(PlayerRegistered.class, PlayerCloudEventsHandlers::handlePlayerRegistered)
 *         .on(FundsDeposited.class, PlayerCloudEventsHandlers::handleFundsDeposited);
 * }
 * }</pre>
 */
public class CloudEventsRouter {
    private final String name;
    private final String inputDomain;
    private final Map<String, Handler<?>> handlers = new HashMap<>();

    private record Handler<T extends Message>(Class<T> eventType, Function<T, CloudEvent> handler) {}

    public CloudEventsRouter(String name, String inputDomain) {
        this.name = name;
        this.inputDomain = inputDomain;
    }

    public String getName() {
        return name;
    }

    public String getInputDomain() {
        return inputDomain;
    }

    /**
     * Register a handler for an event type.
     */
    public <T extends Message> CloudEventsRouter on(Class<T> eventType, Function<T, CloudEvent> handler) {
        String suffix = eventType.getSimpleName();
        handlers.put(suffix, new Handler<>(eventType, handler));
        return this;
    }

    /**
     * Process all events in the book and return CloudEvents.
     */
    public List<CloudEvent> project(EventBook book) {
        List<CloudEvent> events = new ArrayList<>();

        for (EventPage page : book.getPagesList()) {
            if (!page.hasEvent()) continue;

            String suffix = Helpers.typeNameFromUrl(page.getEvent().getTypeUrl());
            Handler<?> handler = handlers.get(suffix);
            if (handler != null) {
                CloudEvent result = dispatch(handler, page.getEvent());
                if (result != null) {
                    events.add(result);
                }
            }
        }
        return events;
    }

    @SuppressWarnings("unchecked")
    private <T extends Message> CloudEvent dispatch(Handler<T> handler, Any eventAny) {
        try {
            T event = eventAny.unpack(handler.eventType());
            return handler.handler().apply(event);
        } catch (Exception e) {
            throw new RuntimeException("Failed to project event", e);
        }
    }
}
