package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import com.google.protobuf.Timestamp;
import dev.angzarr.*;
import dev.angzarr.client.annotations.Applies;
import dev.angzarr.client.annotations.Handles;
import dev.angzarr.client.annotations.Rejected;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.time.Instant;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;

/**
 * Base class for event-sourced aggregates using the OO pattern.
 *
 * Subclasses must:
 * - Override getDomain()
 * - Override createEmptyState()
 * - Annotate command handlers with @Handles(CommandType.class)
 * - Annotate event appliers with @Applies(EventType.class)
 * - Optionally annotate rejection handlers with @Rejected(domain="...", command="...")
 */
public abstract class Aggregate<S> {
    private EventBook eventBook;
    private S state;

    // Dispatch tables built via reflection on first use
    private static final Map<Class<?>, Map<String, MethodInfo>> dispatchTables = new ConcurrentHashMap<>();
    private static final Map<Class<?>, Map<String, MethodInfo>> applierTables = new ConcurrentHashMap<>();
    private static final Map<Class<?>, Map<String, Method>> rejectionTables = new ConcurrentHashMap<>();

    private record MethodInfo(Method method, Class<? extends Message> messageType) {}

    /**
     * The domain this aggregate belongs to.
     */
    public abstract String getDomain();

    /**
     * Create an empty state instance.
     */
    protected abstract S createEmptyState();

    protected Aggregate() {
        this(null);
    }

    protected Aggregate(EventBook eventBook) {
        this.eventBook = eventBook != null ? eventBook : EventBook.getDefaultInstance();
        ensureDispatchTablesBuilt();
    }

    private void ensureDispatchTablesBuilt() {
        var type = getClass();
        if (dispatchTables.containsKey(type)) return;

        synchronized (dispatchTables) {
            if (dispatchTables.containsKey(type)) return;

            var dispatch = new HashMap<String, MethodInfo>();
            var appliers = new HashMap<String, MethodInfo>();
            var rejections = new HashMap<String, Method>();

            for (var method : type.getMethods()) {
                var handles = method.getAnnotation(Handles.class);
                if (handles != null) {
                    dispatch.put(handles.value().getSimpleName(), new MethodInfo(method, handles.value()));
                }

                var applies = method.getAnnotation(Applies.class);
                if (applies != null) {
                    appliers.put(applies.value().getSimpleName(), new MethodInfo(method, applies.value()));
                }

                var rejected = method.getAnnotation(Rejected.class);
                if (rejected != null) {
                    rejections.put(rejected.domain() + "/" + rejected.command(), method);
                }
            }

            dispatchTables.put(type, dispatch);
            applierTables.put(type, appliers);
            rejectionTables.put(type, rejections);
        }
    }

    /**
     * Handle a gRPC request.
     */
    public static <T extends Aggregate<?>> BusinessResponse handle(Class<T> aggClass, ContextualCommand request) {
        try {
            var priorEvents = request.hasEvents() ? request.getEvents() : null;
            var agg = aggClass.getConstructor(EventBook.class).newInstance(priorEvents);

            if (request.getCommand().getPagesList().isEmpty()) {
                throw new Errors.InvalidArgumentError("No command pages");
            }

            var commandAny = request.getCommand().getPages(0).getCommand();

            // Check for Notification
            if (commandAny.getTypeUrl().endsWith("Notification")) {
                var notification = commandAny.unpack(Notification.class);
                return agg.handleRevocation(notification);
            }

            agg.dispatch(commandAny);
            return BusinessResponse.newBuilder().setEvents(agg.getEventBook()).build();
        } catch (Exception e) {
            throw new Errors.ClientError("Failed to handle command", e);
        }
    }

    /**
     * Dispatch a command to the matching @Handles method.
     */
    public void dispatch(Any commandAny) {
        var typeUrl = commandAny.getTypeUrl();
        var dispatch = dispatchTables.get(getClass());

        for (var entry : dispatch.entrySet()) {
            if (typeUrl.endsWith(entry.getKey())) {
                try {
                    var cmd = commandAny.unpack(entry.getValue().messageType());
                    var result = entry.getValue().method().invoke(this, cmd);
                    handleResult(result);
                    return;
                } catch (InvocationTargetException e) {
                    if (e.getCause() instanceof RuntimeException re) throw re;
                    throw new Errors.ClientError("Handler failed", e.getCause());
                } catch (Exception e) {
                    throw new Errors.ClientError("Failed to dispatch command", e);
                }
            }
        }

        throw new Errors.InvalidArgumentError("Unknown command: " + typeUrl);
    }

    /**
     * Handle rejection notification.
     */
    public BusinessResponse handleRevocation(Notification notification) {
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

        var rejectionTable = rejectionTables.get(getClass());
        for (var entry : rejectionTable.entrySet()) {
            var parts = entry.getKey().split("/");
            if (parts[0].equals(domain) && commandSuffix.endsWith(parts[1])) {
                try {
                    getState(); // Ensure state is built
                    var result = entry.getValue().invoke(this, notification);
                    handleResult(result);
                    return BusinessResponse.newBuilder().setEvents(getEventBook()).build();
                } catch (Exception e) {
                    throw new Errors.ClientError("Rejection handler failed", e);
                }
            }
        }

        return BusinessResponse.newBuilder()
            .setRevocation(RevocationResponse.newBuilder()
                .setEmitSystemRevocation(true)
                .setReason("Aggregate " + getDomain() + " has no custom compensation for " + domain + "/" + commandSuffix)
                .build())
            .build();
    }

    private void handleResult(Object result) {
        if (result == null) return;

        if (result instanceof Iterable<?> iterable) {
            for (var item : iterable) {
                if (item instanceof Message msg) {
                    applyAndRecord(msg);
                }
            }
        } else if (result instanceof Message message) {
            applyAndRecord(message);
        }
    }

    /**
     * Get the current state.
     */
    public S getState() {
        if (state == null) {
            state = rebuild();
        }
        return state;
    }

    /**
     * Check if this aggregate has prior events.
     */
    public boolean exists() {
        return state != null || !eventBook.getPagesList().isEmpty();
    }

    /**
     * Get the event book for persistence.
     */
    public EventBook getEventBook() {
        return eventBook;
    }

    /**
     * Build a component descriptor for topology discovery.
     */
    public Descriptor descriptor() {
        var dispatch = dispatchTables.get(getClass());
        return new Descriptor(getDomain(), ComponentTypes.AGGREGATE,
            List.of(new TargetDesc(getDomain(), new ArrayList<>(dispatch.keySet()))));
    }

    private S rebuild() {
        var newState = createEmptyState();
        for (var page : eventBook.getPagesList()) {
            if (page.hasEvent()) {
                applyEvent(newState, page.getEvent());
            }
        }
        // Clear consumed events
        eventBook = EventBook.getDefaultInstance();
        return newState;
    }

    /**
     * Pack event, apply to cached state, add to event book.
     */
    protected void applyAndRecord(Message eventMessage) {
        var eventAny = Any.pack(eventMessage, "type.googleapis.com/");

        if (state != null) {
            applyEvent(state, eventAny);
        }

        var page = EventPage.newBuilder().setEvent(eventAny).build();
        eventBook = eventBook.toBuilder().addPages(page).build();
    }

    /**
     * Apply a single event to state.
     * Override to provide custom event dispatch instead of using @Applies annotations.
     */
    protected void applyEvent(S state, Any eventAny) {
        var appliers = applierTables.get(getClass());
        for (var entry : appliers.entrySet()) {
            if (eventAny.getTypeUrl().endsWith(entry.getKey())) {
                try {
                    var event = eventAny.unpack(entry.getValue().messageType());
                    entry.getValue().method().invoke(this, state, event);
                    return;
                } catch (Exception ignored) {}
            }
        }
        // Unknown event type - silently ignore
    }

    /**
     * Rehydrate state from an event book.
     * Alternative to constructor injection for backward compatibility.
     */
    public void rehydrate(EventBook newEventBook) {
        this.eventBook = newEventBook != null ? newEventBook : EventBook.getDefaultInstance();
        this.state = null; // Force rebuild on next getState()
    }

    /**
     * Handle a command and return the resulting event.
     * Convenience method for testing and simple use cases.
     *
     * @param command The command message to handle
     * @return The resulting event message
     */
    public Message handleCommand(Message command) {
        var commandAny = Any.pack(command, "type.googleapis.com/");
        var typeUrl = commandAny.getTypeUrl();
        var dispatch = dispatchTables.get(getClass());

        for (var entry : dispatch.entrySet()) {
            if (typeUrl.endsWith(entry.getKey())) {
                try {
                    getState(); // Ensure state is built
                    var result = entry.getValue().method().invoke(this, command);
                    if (result instanceof Message msg) {
                        applyAndRecord(msg);
                        return msg;
                    }
                    return null;
                } catch (InvocationTargetException e) {
                    if (e.getCause() instanceof RuntimeException re) throw re;
                    throw new Errors.ClientError("Handler failed", e.getCause());
                } catch (Exception e) {
                    throw new Errors.ClientError("Failed to dispatch command", e);
                }
            }
        }

        throw new Errors.InvalidArgumentError("Unknown command: " + typeUrl);
    }

    /**
     * Create a timestamp for the current instant.
     */
    protected static Timestamp now() {
        Instant instant = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(instant.getEpochSecond())
            .setNanos(instant.getNano())
            .build();
    }
}
