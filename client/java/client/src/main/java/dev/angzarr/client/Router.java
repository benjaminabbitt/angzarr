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
class CommandRouter<S> {
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

/**
 * Unified event dispatcher for sagas, process managers, and projectors.
 * Uses fluent .domain().on() pattern to register handlers with domain context.
 *
 * Example (Saga - single domain):
 * <pre>
 * EventRouter router = new EventRouter("saga-table-hand")
 *     .domain("table")
 *     .on("HandStarted", this::handleStarted);
 * </pre>
 *
 * Example (Process Manager - multi-domain):
 * <pre>
 * EventRouter router = new EventRouter("pmg-order-flow")
 *     .domain("order")
 *     .on("OrderCreated", this::handleCreated)
 *     .domain("inventory")
 *     .on("StockReserved", this::handleReserved);
 * </pre>
 *
 * Example (Projector - multi-domain):
 * <pre>
 * EventRouter router = new EventRouter("prj-output")
 *     .domain("player")
 *     .on("PlayerRegistered", this::handleRegistered)
 *     .domain("hand")
 *     .on("CardsDealt", this::handleDealt);
 * </pre>
 */
class EventRouter {
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
}

/**
 * Fluent state reconstruction from events (functional pattern).
 */
class StateRouter<S> {
    private final java.util.function.Supplier<S> factory;
    private final Map<String, BiFunction<S, Any, Void>> appliers = new HashMap<>();

    public StateRouter(java.util.function.Supplier<S> factory) {
        this.factory = factory;
    }

    public <E extends Message> StateRouter<S> on(Class<E> eventType, java.util.function.BiConsumer<S, E> applier) {
        var suffix = eventType.getSimpleName();
        appliers.put(suffix, (state, any) -> {
            try {
                @SuppressWarnings("unchecked")
                E event = (E) any.unpack(eventType);
                applier.accept(state, event);
            } catch (InvalidProtocolBufferException ignored) {}
            return null;
        });
        return this;
    }

    public S withEventBook(EventBook book) {
        var state = factory.get();
        if (book == null) return state;

        for (var page : book.getPagesList()) {
            if (!page.hasEvent()) continue;
            applyEvent(state, page.getEvent());
        }
        return state;
    }

    private void applyEvent(S state, Any eventAny) {
        for (var entry : appliers.entrySet()) {
            if (eventAny.getTypeUrl().endsWith(entry.getKey())) {
                entry.getValue().apply(state, eventAny);
                return;
            }
        }
    }
}
