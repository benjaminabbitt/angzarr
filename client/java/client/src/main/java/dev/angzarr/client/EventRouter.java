package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.Cover;
import dev.angzarr.CommandBook;
import dev.angzarr.EventBook;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * Unified router for saga event handling.
 * Uses fluent .domain().on() pattern to register handlers with domain context.
 *
 * <p>Example:
 * <pre>{@code
 * EventRouter router = new EventRouter("saga-table-hand")
 *     .domain("table")
 *     .prepare(HandStarted.class, MyHandler::prepareHandStarted)
 *     .on(HandStarted.class, MyHandler::handleHandStarted);
 *
 * // Prepare phase
 * List<Cover> destinations = router.doPrepare(eventMessage);
 *
 * // Execute phase
 * CommandBook result = router.doHandle(eventMessage, destinationBooks);
 * }</pre>
 */
public class EventRouter {

    private static final String TYPE_URL_PREFIX = "type.googleapis.com/";

    private final String name;
    private String currentDomain;
    private final Map<Class<?>, PrepareRegistration<?>> prepareHandlers = new HashMap<>();
    private final Map<Class<?>, ReactRegistration<?>> reactHandlers = new HashMap<>();

    /**
     * Create a new EventRouter with the given component name.
     *
     * @param name The component name (e.g., "saga-table-hand")
     */
    public EventRouter(String name) {
        this.name = name;
    }

    /**
     * Get the router name.
     */
    public String getName() {
        return name;
    }

    /**
     * Get the current domain context.
     */
    public String getCurrentDomain() {
        return currentDomain;
    }

    /**
     * Set the current domain context for subsequent on() calls.
     *
     * @param name The domain name
     * @return this router for fluent chaining
     */
    public EventRouter domain(String name) {
        this.currentDomain = name;
        return this;
    }

    /**
     * Register a prepare handler for an event type.
     * Must be called after domain() to set context.
     *
     * <p>The prepare handler declares which destinations are needed
     * before the execute phase.
     *
     * @param eventClass The event class to handle
     * @param handler Function that takes the event and returns destination covers
     * @param <T> The event type
     * @return this router for fluent chaining
     */
    public <T extends Message> EventRouter prepare(
            Class<T> eventClass,
            Function<T, List<Cover>> handler) {
        if (currentDomain == null) {
            throw new IllegalStateException("Must call domain() before prepare()");
        }
        prepareHandlers.put(eventClass, new PrepareRegistration<>(eventClass, handler));
        return this;
    }

    /**
     * Register an event reaction handler.
     * Must be called after domain() to set context.
     *
     * <p>The handler receives the event and fetched destination EventBooks,
     * and returns CommandBooks to execute on other aggregates.
     *
     * @param eventClass The event class to handle
     * @param handler Function that takes (event, destinations) and returns a CommandBook
     * @param <T> The event type
     * @return this router for fluent chaining
     */
    public <T extends Message> EventRouter on(
            Class<T> eventClass,
            BiFunction<T, List<EventBook>, CommandBook> handler) {
        if (currentDomain == null) {
            throw new IllegalStateException("Must call domain() before on()");
        }
        reactHandlers.put(eventClass, new ReactRegistration<>(eventClass, handler));
        return this;
    }

    /**
     * Execute prepare phase for an event.
     *
     * @param eventMessage The event message
     * @return List of covers for destinations that need to be fetched
     */
    public List<Cover> doPrepare(Message eventMessage) {
        @SuppressWarnings("unchecked")
        PrepareRegistration<Message> reg =
                (PrepareRegistration<Message>) prepareHandlers.get(eventMessage.getClass());
        if (reg != null) {
            return reg.handler.apply(eventMessage);
        }
        return new ArrayList<>();
    }

    /**
     * Execute prepare phase for an event wrapped in Any.
     * Automatically unpacks based on registered handlers.
     *
     * @param eventAny The event wrapped in Any
     * @return List of covers for destinations that need to be fetched
     */
    public List<Cover> doPrepareAny(Any eventAny) {
        for (var entry : prepareHandlers.entrySet()) {
            if (matchesTypeUrl(eventAny.getTypeUrl(), entry.getKey())) {
                try {
                    Message event = eventAny.unpack(entry.getValue().eventClass);
                    @SuppressWarnings("unchecked")
                    PrepareRegistration<Message> reg = (PrepareRegistration<Message>) entry.getValue();
                    return reg.handler.apply(event);
                } catch (InvalidProtocolBufferException e) {
                    throw new RuntimeException("Failed to unpack event", e);
                }
            }
        }
        return new ArrayList<>();
    }

    /**
     * Execute handle phase for an event.
     *
     * @param eventMessage The event message
     * @param destinations The fetched destination EventBooks
     * @return CommandBook to execute, or null if no handler matched
     */
    public CommandBook doHandle(Message eventMessage, List<EventBook> destinations) {
        @SuppressWarnings("unchecked")
        ReactRegistration<Message> reg =
                (ReactRegistration<Message>) reactHandlers.get(eventMessage.getClass());
        if (reg != null) {
            return reg.handler.apply(eventMessage, destinations);
        }
        return null;
    }

    /**
     * Execute handle phase for an event wrapped in Any.
     * Automatically unpacks based on registered handlers.
     *
     * @param eventAny The event wrapped in Any
     * @param destinations The fetched destination EventBooks
     * @return CommandBook to execute, or null if no handler matched
     */
    public CommandBook doHandleAny(Any eventAny, List<EventBook> destinations) {
        for (var entry : reactHandlers.entrySet()) {
            if (matchesTypeUrl(eventAny.getTypeUrl(), entry.getKey())) {
                try {
                    Message event = eventAny.unpack(entry.getValue().eventClass);
                    @SuppressWarnings("unchecked")
                    ReactRegistration<Message> reg = (ReactRegistration<Message>) entry.getValue();
                    return reg.handler.apply(event, destinations);
                } catch (InvalidProtocolBufferException e) {
                    throw new RuntimeException("Failed to unpack event", e);
                }
            }
        }
        return null;
    }

    /**
     * Execute prepare phase for source EventBook.
     *
     * @param source The source EventBook
     * @return List of covers for destinations that need to be fetched
     */
    public List<Cover> prepareDestinations(EventBook source) {
        if (source == null || source.getPagesList().isEmpty()) {
            return new ArrayList<>();
        }
        var lastPage = source.getPages(source.getPagesCount() - 1);
        if (!lastPage.hasEvent()) {
            return new ArrayList<>();
        }
        return doPrepareAny(lastPage.getEvent());
    }

    /**
     * Execute dispatch for source EventBook.
     *
     * @param source The source EventBook
     * @param destinations The fetched destination EventBooks
     * @return List of CommandBooks to send
     */
    public List<CommandBook> dispatch(EventBook source, List<EventBook> destinations) {
        if (source == null || source.getPagesList().isEmpty()) {
            return new ArrayList<>();
        }
        var lastPage = source.getPages(source.getPagesCount() - 1);
        if (!lastPage.hasEvent()) {
            return new ArrayList<>();
        }
        var result = doHandleAny(lastPage.getEvent(), destinations);
        if (result != null) {
            return List.of(result);
        }
        return new ArrayList<>();
    }

    private boolean matchesTypeUrl(String typeUrl, Class<?> eventClass) {
        try {
            Message instance = (Message) eventClass.getDeclaredMethod("getDefaultInstance").invoke(null);
            String fullName = instance.getDescriptorForType().getFullName();
            return typeUrl.endsWith(fullName);
        } catch (Exception e) {
            return false;
        }
    }

    /**
     * Get the registered event types.
     *
     * @return List of fully-qualified event type names
     */
    public List<String> getEventTypes() {
        List<String> types = new ArrayList<>();
        for (Class<?> clazz : reactHandlers.keySet()) {
            try {
                Message instance = (Message) clazz.getDeclaredMethod("getDefaultInstance").invoke(null);
                types.add(instance.getDescriptorForType().getFullName());
            } catch (Exception e) {
                // Skip if we can't get the type name
            }
        }
        return types;
    }

    // ========================================================================
    // Static helpers
    // ========================================================================

    /**
     * Get next sequence number from an event book.
     *
     * <p>Used when building commands to set the correct sequence number
     * for optimistic concurrency.
     *
     * @param eventBook The event book (may be null)
     * @return The next sequence number (0 if eventBook is null or empty)
     */
    public static int nextSequence(EventBook eventBook) {
        if (eventBook == null) {
            return 0;
        }
        return (int) eventBook.getNextSequence();
    }

    /**
     * Pack a command message into Any.
     *
     * @param command The command message to pack
     * @return Any containing the packed command
     */
    public static Any packCommand(Message command) {
        return Any.pack(command, TYPE_URL_PREFIX);
    }

    /**
     * Unpack an event from Any to a specific type.
     *
     * @param eventAny The Any containing the event
     * @param eventClass The expected event class
     * @param <T> The event type
     * @return The unpacked event
     * @throws InvalidProtocolBufferException if unpacking fails
     */
    public static <T extends Message> T unpackEvent(Any eventAny, Class<T> eventClass)
            throws InvalidProtocolBufferException {
        return eventAny.unpack(eventClass);
    }

    // ========================================================================
    // Internal registration classes
    // ========================================================================

    private static class PrepareRegistration<T extends Message> {
        final Class<T> eventClass;
        final Function<T, List<Cover>> handler;

        PrepareRegistration(Class<T> eventClass, Function<T, List<Cover>> handler) {
            this.eventClass = eventClass;
            this.handler = handler;
        }
    }

    private static class ReactRegistration<T extends Message> {
        final Class<T> eventClass;
        final BiFunction<T, List<EventBook>, CommandBook> handler;

        ReactRegistration(Class<T> eventClass, BiFunction<T, List<EventBook>, CommandBook> handler) {
            this.eventClass = eventClass;
            this.handler = handler;
        }
    }
}
