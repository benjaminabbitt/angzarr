package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;

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
        EventBook handle(Notification notification, S state);
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
                var events = entry.getValue().handle(notification, state);
                return BusinessResponse.newBuilder().setEvents(events).build();
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

    public Descriptor descriptor() {
        return new Descriptor(domain, ComponentTypes.AGGREGATE,
            List.of(new TargetDesc(domain, types())));
    }

    public List<String> types() {
        return handlers.stream().map(Map.Entry::getKey).toList();
    }
}

/**
 * DRY event dispatcher for sagas (functional pattern).
 */
class EventRouter {
    private final String name;
    private final String inputDomain;
    private final Map<String, List<String>> outputTargets = new HashMap<>();
    private final List<Map.Entry<String, EventHandler>> handlers = new ArrayList<>();
    private final Map<String, PrepareHandler> prepareHandlers = new HashMap<>();

    @FunctionalInterface
    public interface EventHandler {
        List<CommandBook> handle(Any eventAny, byte[] root, String correlationId, List<EventBook> destinations);
    }

    @FunctionalInterface
    public interface PrepareHandler {
        List<Cover> handle(Any eventAny, dev.angzarr.UUID root);
    }

    public EventRouter(String name, String inputDomain) {
        this.name = name;
        this.inputDomain = inputDomain;
    }

    public EventRouter sends(String domain, String commandType) {
        outputTargets.computeIfAbsent(domain, k -> new ArrayList<>()).add(commandType);
        return this;
    }

    public EventRouter prepare(String suffix, PrepareHandler handler) {
        prepareHandlers.put(suffix, handler);
        return this;
    }

    public EventRouter on(String suffix, EventHandler handler) {
        handlers.add(new AbstractMap.SimpleEntry<>(suffix, handler));
        return this;
    }

    public List<Cover> prepareDestinations(EventBook book) {
        var root = book.hasCover() ? book.getCover().getRoot() : null;
        var destinations = new ArrayList<Cover>();

        for (var page : book.getPagesList()) {
            if (!page.hasEvent()) continue;
            for (var entry : prepareHandlers.entrySet()) {
                if (page.getEvent().getTypeUrl().endsWith(entry.getKey())) {
                    destinations.addAll(entry.getValue().handle(page.getEvent(), root));
                    break;
                }
            }
        }
        return destinations;
    }

    public List<CommandBook> dispatch(EventBook book, List<EventBook> destinations) {
        var root = book.hasCover() ? book.getCover().getRoot().getValue().toByteArray() : null;
        var correlationId = book.hasCover() ? book.getCover().getCorrelationId() : "";
        var dests = destinations != null ? destinations : List.<EventBook>of();

        var commands = new ArrayList<CommandBook>();
        for (var page : book.getPagesList()) {
            if (!page.hasEvent()) continue;
            for (var entry : handlers) {
                if (page.getEvent().getTypeUrl().endsWith(entry.getKey())) {
                    commands.addAll(entry.getValue().handle(page.getEvent(), root, correlationId, dests));
                    break;
                }
            }
        }
        return commands;
    }

    public Descriptor descriptor() {
        return new Descriptor(name, ComponentTypes.SAGA,
            List.of(new TargetDesc(inputDomain, types())));
    }

    public List<String> types() {
        return handlers.stream().map(Map.Entry::getKey).toList();
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
