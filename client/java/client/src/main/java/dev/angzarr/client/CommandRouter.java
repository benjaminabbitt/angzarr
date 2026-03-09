package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.PageHeader;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

/**
 * Fluent router for aggregate command handling.
 *
 * <p>Provides a simple builder pattern for registering command handlers.
 * Uses functional interfaces for state rebuilding and command handling.
 *
 * <p>Example:
 * <pre>{@code
 * CommandRouter<PlayerState> router = new CommandRouter<>("player", StateBuilder::fromEventBook)
 *     .on("RegisterPlayer", RegisterPlayer.class, RegisterHandler::handle)
 *     .on("DepositFunds", DepositFunds.class, DepositHandler::handle)
 *     .on("WithdrawFunds", WithdrawFunds.class, WithdrawHandler::handle);
 *
 * // Dispatch a command
 * EventBook result = router.dispatch(contextualCommand);
 * }</pre>
 *
 * @param <S> The state type for this aggregate
 */
public class CommandRouter<S> {

    private static final String TYPE_URL_PREFIX = "type.googleapis.com/";

    private final String domain;
    private final Function<EventBook, S> stateBuilder;
    private final Map<Class<?>, HandlerRegistration<?>> handlers = new HashMap<>();

    /**
     * Create a new CommandRouter for an aggregate.
     *
     * @param domain The domain name for this aggregate
     * @param stateBuilder Function to rebuild state from events
     */
    public CommandRouter(String domain, Function<EventBook, S> stateBuilder) {
        this.domain = domain;
        this.stateBuilder = stateBuilder;
    }

    /**
     * Get the domain name.
     */
    public String getDomain() {
        return domain;
    }

    /**
     * Register a command handler.
     *
     * @param commandName The command type name (suffix, e.g., "RegisterPlayer")
     * @param commandClass The protobuf command class
     * @param handler The handler function
     * @param <C> The command type
     * @return this router for fluent chaining
     */
    public <C extends Message> CommandRouter<S> on(
            String commandName,
            Class<C> commandClass,
            CommandHandler<C, S> handler) {
        handlers.put(commandClass, new HandlerRegistration<>(commandName, commandClass, handler));
        return this;
    }

    /**
     * Get the registered command types.
     *
     * @return List of command type names
     */
    public List<String> getCommandTypes() {
        return handlers.values().stream()
                .map(reg -> reg.commandName)
                .toList();
    }

    /**
     * Dispatch a contextual command and return resulting events.
     *
     * @param cmd The contextual command containing command and prior events
     * @return The event book containing resulting events
     * @throws RouterException if dispatch fails
     */
    public EventBook dispatch(ContextualCommand cmd) throws RouterException {
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
        S state = stateBuilder.apply(eventBook);
        int seq = Helpers.nextSequence(eventBook);

        // Find handler by type URL
        for (var entry : handlers.entrySet()) {
            if (matchesTypeUrl(commandAny.getTypeUrl(), entry.getKey())) {
                return dispatchToHandler(entry.getValue(), commandAny, commandBook, state, seq);
            }
        }

        throw new RouterException("Unknown command type: " + commandAny.getTypeUrl());
    }

    @SuppressWarnings("unchecked")
    private <C extends Message> EventBook dispatchToHandler(
            HandlerRegistration<?> registration,
            Any commandAny,
            CommandBook commandBook,
            S state,
            int seq) throws RouterException {
        try {
            HandlerRegistration<C> reg = (HandlerRegistration<C>) registration;
            C command = commandAny.unpack(reg.commandClass);
            CommandHandler<C, S> handler = (CommandHandler<C, S>) reg.handler;
            Message event = handler.handle(command, state);

            // Wrap the event into an EventBook
            return EventBook.newBuilder()
                    .setCover(commandBook.getCover())
                    .addPages(EventPage.newBuilder()
                            .setHeader(PageHeader.newBuilder().setSequence(seq).build())
                            .setEvent(Any.pack(event, TYPE_URL_PREFIX))
                            .build())
                    .build();
        } catch (InvalidProtocolBufferException e) {
            throw new RouterException("Failed to unpack command", e);
        } catch (CommandRejectedException e) {
            throw new RouterException("Command rejected: " + e.getMessage(), e);
        }
    }

    private boolean matchesTypeUrl(String typeUrl, Class<?> messageClass) {
        try {
            Message instance = (Message) messageClass.getDeclaredMethod("getDefaultInstance").invoke(null);
            String fullName = instance.getDescriptorForType().getFullName();
            return typeUrl.endsWith(fullName);
        } catch (Exception e) {
            return false;
        }
    }

    /**
     * Functional interface for command handlers.
     *
     * <p>Handlers follow the guard/validate/compute pattern:
     * - Guard: Check state preconditions
     * - Validate: Validate command inputs
     * - Compute: Build the resulting event
     *
     * @param <C> The command type
     * @param <S> The state type
     */
    @FunctionalInterface
    public interface CommandHandler<C extends Message, S> {
        /**
         * Handle a command and return the resulting event.
         *
         * @param command The unpacked command
         * @param state The current state
         * @return The resulting event (will be wrapped in EventBook)
         * @throws CommandRejectedException if the command should be rejected
         */
        Message handle(C command, S state) throws CommandRejectedException;
    }

    /**
     * Exception thrown when a command is rejected.
     */
    public static class CommandRejectedException extends Exception {
        public CommandRejectedException(String message) {
            super(message);
        }

        public CommandRejectedException(String message, Throwable cause) {
            super(message, cause);
        }
    }

    /**
     * Exception thrown for router errors.
     */
    public static class RouterException extends Exception {
        public RouterException(String message) {
            super(message);
        }

        public RouterException(String message, Throwable cause) {
            super(message, cause);
        }
    }

    // ========================================================================
    // Static helpers
    // ========================================================================

    /**
     * Pack an event message into Any.
     *
     * @param event The event message to pack
     * @return Any containing the packed event
     */
    public static Any packEvent(Message event) {
        return Any.pack(event, TYPE_URL_PREFIX);
    }

    // ========================================================================
    // Internal registration class
    // ========================================================================

    private static class HandlerRegistration<C extends Message> {
        final String commandName;
        final Class<C> commandClass;
        final CommandHandler<C, ?> handler;

        HandlerRegistration(String commandName, Class<C> commandClass, CommandHandler<C, ?> handler) {
            this.commandName = commandName;
            this.commandClass = commandClass;
            this.handler = handler;
        }
    }
}
