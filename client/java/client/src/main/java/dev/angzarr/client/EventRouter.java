package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;

import java.util.*;

/**
 * Unified event dispatcher for sagas, process managers, and projectors.
 * Uses fluent .domain().on() pattern to register handlers with domain context.
 *
 * Example (Saga - single domain with typed handlers):
 * <pre>
 * EventRouter router = new EventRouter("saga-table-hand")
 *     .domain("table")
 *     .prepare(HandStarted.class, this::prepareHandStarted)
 *     .on(HandStarted.class, this::handleHandStarted);
 * </pre>
 *
 * Example (Process Manager - multi-domain):
 * <pre>
 * EventRouter router = new EventRouter("pmg-order-flow")
 *     .domain("order")
 *     .on(OrderCreated.class, this::handleCreated)
 *     .domain("inventory")
 *     .on(StockReserved.class, this::handleReserved);
 * </pre>
 *
 * Example (Projector - multi-domain):
 * <pre>
 * EventRouter router = new EventRouter("prj-output")
 *     .domain("player")
 *     .on(PlayerRegistered.class, this::handleRegistered)
 *     .domain("hand")
 *     .on(CardsDealt.class, this::handleDealt);
 * </pre>
 */
public class EventRouter {
    private final String name;
    private String currentDomain;
    private final Map<String, List<Map.Entry<String, EventHandler>>> handlers = new HashMap<>();
    private final Map<String, Map<String, PrepareHandler>> prepareHandlers = new HashMap<>();

    @FunctionalInterface
    public interface EventHandler {
        List<CommandBook> handle(Any eventAny, byte[] root, String correlationId, List<EventBook> destinations);
    }

    @FunctionalInterface
    public interface PrepareHandler {
        List<Cover> handle(Any eventAny, dev.angzarr.UUID root);
    }

    /**
     * Typed prepare handler - receives unpacked event, returns list of destination covers.
     */
    @FunctionalInterface
    public interface TypedPrepareHandler<E extends Message> {
        List<Cover> handle(E event);
    }

    /**
     * Typed event handler - receives unpacked event and destinations, returns single CommandBook (or null).
     */
    @FunctionalInterface
    public interface TypedEventHandler<E extends Message> {
        CommandBook handle(E event, List<EventBook> destinations);
    }

    public EventRouter(String name) {
        this.name = name;
    }

    /**
     * Create a new EventRouter with a single input domain (backwards compatibility).
     * @deprecated Use new EventRouter(name).domain(inputDomain) instead.
     */
    @Deprecated
    public EventRouter(String name, String inputDomain) {
        this.name = name;
        if (inputDomain != null && !inputDomain.isEmpty()) {
            domain(inputDomain);
        }
    }

    /**
     * Set the current domain context for subsequent on() calls.
     */
    public EventRouter domain(String name) {
        this.currentDomain = name;
        handlers.computeIfAbsent(name, k -> new ArrayList<>());
        prepareHandlers.computeIfAbsent(name, k -> new HashMap<>());
        return this;
    }

    /**
     * Register a prepare handler for an event type_url suffix.
     * Must be called after domain() to set context.
     */
    public EventRouter prepare(String suffix, PrepareHandler handler) {
        if (currentDomain == null) {
            throw new IllegalStateException("Must call domain() before prepare()");
        }
        prepareHandlers.get(currentDomain).put(suffix, handler);
        return this;
    }

    /**
     * Register a typed prepare handler for an event type.
     * Must be called after domain() to set context.
     * The handler receives the unpacked event directly.
     */
    @SuppressWarnings("unchecked")
    public <E extends Message> EventRouter prepare(Class<E> eventType, TypedPrepareHandler<E> handler) {
        if (currentDomain == null) {
            throw new IllegalStateException("Must call domain() before prepare()");
        }
        String suffix = eventType.getSimpleName();
        prepareHandlers.get(currentDomain).put(suffix, (eventAny, root) -> {
            try {
                E event = (E) eventAny.unpack(eventType);
                return handler.handle(event);
            } catch (InvalidProtocolBufferException e) {
                return List.of();
            }
        });
        return this;
    }

    /**
     * Register a handler for an event type_url suffix in current domain.
     * Must be called after domain() to set context.
     */
    public EventRouter on(String suffix, EventHandler handler) {
        if (currentDomain == null) {
            throw new IllegalStateException("Must call domain() before on()");
        }
        handlers.get(currentDomain).add(new AbstractMap.SimpleEntry<>(suffix, handler));
        return this;
    }

    /**
     * Register a typed handler for an event type in current domain.
     * Must be called after domain() to set context.
     * The handler receives the unpacked event directly.
     */
    @SuppressWarnings("unchecked")
    public <E extends Message> EventRouter on(Class<E> eventType, TypedEventHandler<E> handler) {
        if (currentDomain == null) {
            throw new IllegalStateException("Must call domain() before on()");
        }
        String suffix = eventType.getSimpleName();
        handlers.get(currentDomain).add(new AbstractMap.SimpleEntry<>(suffix, (eventAny, root, correlationId, destinations) -> {
            try {
                E event = (E) eventAny.unpack(eventType);
                CommandBook result = handler.handle(event, destinations);
                if (result == null) {
                    return List.of();
                }
                return List.of(result);
            } catch (InvalidProtocolBufferException e) {
                return List.of();
            }
        }));
        return this;
    }

    /**
     * Auto-derive subscriptions from registered handlers.
     * @return Map of domain to event types.
     */
    public Map<String, List<String>> subscriptions() {
        var result = new HashMap<String, List<String>>();
        for (var entry : handlers.entrySet()) {
            if (!entry.getValue().isEmpty()) {
                result.put(entry.getKey(),
                    entry.getValue().stream().map(Map.Entry::getKey).toList());
            }
        }
        return result;
    }

    /**
     * Get destinations needed for the given source events.
     * Routes based on source domain.
     */
    public List<Cover> prepareDestinations(EventBook book) {
        var sourceDomain = book.hasCover() ? book.getCover().getDomain() : "";
        var domainHandlers = prepareHandlers.get(sourceDomain);
        if (domainHandlers == null) {
            return List.of();
        }

        var root = book.hasCover() ? book.getCover().getRoot() : null;
        var destinations = new ArrayList<Cover>();

        for (var page : book.getPagesList()) {
            if (!page.hasEvent()) continue;
            for (var entry : domainHandlers.entrySet()) {
                if (page.getEvent().getTypeUrl().endsWith(entry.getKey())) {
                    destinations.addAll(entry.getValue().handle(page.getEvent(), root));
                    break;
                }
            }
        }
        return destinations;
    }

    /**
     * Dispatch all events in an EventBook to registered handlers.
     * Routes based on source domain and event type suffix.
     */
    public List<CommandBook> dispatch(EventBook book, List<EventBook> destinations) {
        var sourceDomain = book.hasCover() ? book.getCover().getDomain() : "";
        var domainHandlers = handlers.get(sourceDomain);
        if (domainHandlers == null) {
            return List.of();
        }

        var root = book.hasCover() ? book.getCover().getRoot().getValue().toByteArray() : null;
        var correlationId = book.hasCover() ? book.getCover().getCorrelationId() : "";
        var dests = destinations != null ? destinations : List.<EventBook>of();

        var commands = new ArrayList<CommandBook>();
        for (var page : book.getPagesList()) {
            if (!page.hasEvent()) continue;
            for (var entry : domainHandlers) {
                if (page.getEvent().getTypeUrl().endsWith(entry.getKey())) {
                    commands.addAll(entry.getValue().handle(page.getEvent(), root, correlationId, dests));
                    break;
                }
            }
        }
        return commands;
    }

    /**
     * Return the first registered domain (for backwards compatibility).
     * @deprecated Use subscriptions() instead.
     */
    @Deprecated
    public String inputDomain() {
        return handlers.keySet().stream().findFirst().orElse("");
    }

    /**
     * Declare an output domain and command type (deprecated, no-op).
     * This method was used for topology discovery but is now deprecated.
     * @deprecated This method has no effect and will be removed.
     */
    @Deprecated
    public EventRouter sends(String domain, String commandType) {
        // No-op for backwards compatibility
        return this;
    }

    /**
     * Return output domain names (deprecated, returns empty list).
     * @deprecated Output domains are no longer tracked.
     */
    @Deprecated
    public List<String> outputDomains() {
        return List.of();
    }

    /**
     * Return command types for a given output domain (deprecated, returns empty list).
     * @deprecated Output types are no longer tracked.
     */
    @Deprecated
    public List<String> outputTypes(String domain) {
        return List.of();
    }

    /**
     * Calculate next sequence number from an EventBook.
     */
    public static int nextSequence(EventBook eventBook) {
        return Helpers.nextSequence(eventBook);
    }

    /**
     * Pack a command message as Any.
     */
    public static Any packCommand(com.google.protobuf.Message message) {
        return Any.pack(message, "type.googleapis.com/");
    }
}
