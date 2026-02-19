package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.client.annotations.Applies;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.annotations.ReactsTo;
import dev.angzarr.client.annotations.Rejected;
import dev.angzarr.client.compensation.RejectionHandlerResponse;

import java.lang.reflect.Method;
import java.util.*;
import java.util.function.BiConsumer;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * Base class for process managers using annotation-based handler registration.
 *
 * <p>Process managers are stateful coordinators that accept events from multiple
 * domains and emit commands. They use correlation IDs as aggregate roots.
 *
 * <p>Usage:
 * <pre>{@code
 * public class HandFlowPM extends ProcessManager<HandFlowState> {
 *     public HandFlowPM() {
 *         super("hand-flow");
 *     }
 *
 *     @Override
 *     protected HandFlowState createEmptyState() {
 *         return HandFlowState.newBuilder().build();
 *     }
 *
 *     @ReactsTo(HandStarted.class)
 *     public List<CommandBook> handleHandStarted(HandStarted event) {
 *         // Use getState() to access current PM state
 *         return packCommands("player", DeductBuyIn.newBuilder()...build(), correlationId);
 *     }
 *
 *     @Applies(HandStarted.class)
 *     public void applyHandStarted(HandFlowState.Builder state, HandStarted event) {
 *         state.setPhase("started");
 *     }
 *
 *     @Rejected(domain = "player", commandType = DeductBuyIn.class)
 *     public EventBook handleRejectedDeductBuyIn(Notification notification) {
 *         // Handle compensation
 *     }
 * }
 * }</pre>
 *
 * @param <S> The state type (must be a protobuf message)
 */
public abstract class ProcessManager<S extends Message> {
    private final String name;
    private final Map<String, BiFunction<Any, String, List<CommandBook>>> handlers = new HashMap<>();
    private final Map<String, BiConsumer<S, Any>> appliers = new HashMap<>();
    private final Map<String, BiFunction<Notification, S, RejectionHandlerResponse>> rejectionHandlers = new HashMap<>();
    private final Map<String, Function<Any, List<Cover>>> prepareHandlers = new HashMap<>();

    private S state;
    private boolean exists = false;

    protected ProcessManager(String name) {
        this.name = name;
        buildDispatchTables();
    }

    public String getName() {
        return name;
    }

    /**
     * Create an empty state instance.
     */
    protected abstract S createEmptyState();

    /**
     * Result from PM dispatch - commands for normal events, or rejection response.
     */
    public static class DispatchResult {
        private final List<CommandBook> commands;
        private final RejectionHandlerResponse rejectionResponse;

        public DispatchResult(List<CommandBook> commands, RejectionHandlerResponse rejectionResponse) {
            this.commands = commands;
            this.rejectionResponse = rejectionResponse;
        }

        public List<CommandBook> getCommands() {
            return commands;
        }

        public RejectionHandlerResponse getRejectionResponse() {
            return rejectionResponse;
        }

        public boolean hasRejectionResponse() {
            return rejectionResponse != null;
        }
    }

    /**
     * Dispatch events and produce commands.
     */
    public DispatchResult dispatch(EventBook book, EventBook priorEvents) {
        rebuildState(priorEvents);

        String correlationId = book.hasCover() ? book.getCover().getCorrelationId() : "";
        if (correlationId.isEmpty()) {
            // PMs require correlation ID
            return new DispatchResult(List.of(), null);
        }

        List<CommandBook> commands = new ArrayList<>();
        for (EventPage page : book.getPagesList()) {
            if (!page.hasEvent()) continue;

            // Check for rejection notification
            if (Helpers.typeUrlMatches(page.getEvent().getTypeUrl(), "Notification")) {
                try {
                    Notification notification = page.getEvent().unpack(Notification.class);
                    RejectionHandlerResponse response = dispatchRejection(notification);
                    return new DispatchResult(List.of(), response);
                } catch (InvalidProtocolBufferException e) {
                    // Ignore malformed notifications
                }
                continue;
            }

            String suffix = Helpers.typeNameFromUrl(page.getEvent().getTypeUrl());

            // Apply event to state
            BiConsumer<S, Any> applier = appliers.get(suffix);
            if (applier != null) {
                applier.accept(state, page.getEvent());
            }

            // Dispatch to handler
            BiFunction<Any, String, List<CommandBook>> handler = handlers.get(suffix);
            if (handler != null) {
                commands.addAll(handler.apply(page.getEvent(), correlationId));
            }
        }
        return new DispatchResult(commands, null);
    }

    /**
     * Get destinations needed for trigger events (two-phase protocol).
     */
    public List<Cover> prepareDestinations(EventBook trigger, EventBook priorEvents) {
        rebuildState(priorEvents);

        List<Cover> destinations = new ArrayList<>();
        for (EventPage page : trigger.getPagesList()) {
            if (!page.hasEvent()) continue;

            String suffix = Helpers.typeNameFromUrl(page.getEvent().getTypeUrl());
            Function<Any, List<Cover>> handler = prepareHandlers.get(suffix);
            if (handler != null) {
                destinations.addAll(handler.apply(page.getEvent()));
            }
        }
        return destinations;
    }

    /**
     * Check if the PM exists.
     */
    public boolean exists() {
        return exists;
    }

    /**
     * Get the current state.
     */
    public S getState() {
        return state;
    }

    /**
     * Pack commands for output.
     */
    protected List<CommandBook> packCommands(String domain, Message command, String correlationId) {
        CommandBook.Builder builder = CommandBook.newBuilder();
        Cover.Builder cover = Cover.newBuilder()
            .setDomain(domain)
            .setCorrelationId(correlationId);
        builder.setCover(cover);

        CommandPage.Builder page = CommandPage.newBuilder();
        page.setCommand(Any.pack(command, "type.googleapis.com/"));
        builder.addPages(page);

        return List.of(builder.build());
    }

    private void rebuildState(EventBook eventBook) {
        state = createEmptyState();
        exists = false;

        if (eventBook == null) return;

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) continue;

            String suffix = Helpers.typeNameFromUrl(page.getEvent().getTypeUrl());
            BiConsumer<S, Any> applier = appliers.get(suffix);
            if (applier != null) {
                applier.accept(state, page.getEvent());
                exists = true;
            }
        }
    }

    private RejectionHandlerResponse dispatchRejection(Notification notification) {
        String domain = "";
        String commandSuffix = "";

        if (notification.hasPayload()) {
            try {
                RejectionNotification rejection = notification.getPayload()
                    .unpack(RejectionNotification.class);
                if (rejection.hasRejectedCommand() &&
                    rejection.getRejectedCommand().getPagesCount() > 0) {
                    CommandBook rejectedCmd = rejection.getRejectedCommand();
                    domain = rejectedCmd.getCover().getDomain();
                    commandSuffix = Helpers.typeNameFromUrl(
                        rejectedCmd.getPages(0).getCommand().getTypeUrl());
                }
            } catch (InvalidProtocolBufferException e) {
                // Ignore malformed rejections
            }
        }

        String key = domain + "/" + commandSuffix;
        BiFunction<Notification, S, RejectionHandlerResponse> handler = rejectionHandlers.get(key);
        if (handler != null) {
            return handler.apply(notification, state);
        }

        // Default: no handler found
        return RejectionHandlerResponse.empty();
    }

    @SuppressWarnings("unchecked")
    private void buildDispatchTables() {
        for (Method method : this.getClass().getDeclaredMethods()) {
            // ReactsTo handlers
            ReactsTo reactsTo = method.getAnnotation(ReactsTo.class);
            if (reactsTo != null) {
                Class<? extends Message> eventType = reactsTo.value();
                String suffix = eventType.getSimpleName();
                method.setAccessible(true);

                handlers.put(suffix, (eventAny, correlationId) -> {
                    try {
                        Message event = eventAny.unpack(eventType);
                        Object result = method.invoke(this, event);
                        return packResult(result, correlationId);
                    } catch (Exception e) {
                        throw new RuntimeException("Failed to invoke handler for " + suffix, e);
                    }
                });
            }

            // Applies handlers
            Applies applies = method.getAnnotation(Applies.class);
            if (applies != null) {
                Class<? extends Message> eventType = applies.value();
                String suffix = eventType.getSimpleName();
                method.setAccessible(true);

                appliers.put(suffix, (currentState, eventAny) -> {
                    try {
                        Message event = eventAny.unpack(eventType);
                        method.invoke(this, currentState, event);
                    } catch (Exception e) {
                        throw new RuntimeException("Failed to apply event " + suffix, e);
                    }
                });
            }

            // Rejected handlers
            Rejected rejected = method.getAnnotation(Rejected.class);
            if (rejected != null) {
                String key = rejected.domain() + "/" + rejected.command();
                method.setAccessible(true);

                rejectionHandlers.put(key, (notification, currentState) -> {
                    try {
                        Object result = method.invoke(this, notification);
                        // Handler may return RejectionHandlerResponse directly
                        if (result instanceof RejectionHandlerResponse) {
                            return (RejectionHandlerResponse) result;
                        }
                        // Handler returned EventBook - wrap it
                        if (result instanceof EventBook) {
                            return RejectionHandlerResponse.withEvents((EventBook) result);
                        }
                        // Handler returned null or void
                        return RejectionHandlerResponse.empty();
                    } catch (Exception e) {
                        throw new RuntimeException("Failed to handle rejection for " + key, e);
                    }
                });
            }

            // Prepares handlers
            Prepares prepares = method.getAnnotation(Prepares.class);
            if (prepares != null) {
                Class<? extends Message> eventType = prepares.value();
                String suffix = eventType.getSimpleName();
                method.setAccessible(true);

                prepareHandlers.put(suffix, (eventAny) -> {
                    try {
                        Message event = eventAny.unpack(eventType);
                        Object result = method.invoke(this, event);
                        return asCovers(result);
                    } catch (Exception e) {
                        throw new RuntimeException("Failed to invoke prepare handler for " + suffix, e);
                    }
                });
            }
        }
    }

    @SuppressWarnings("unchecked")
    private List<Cover> asCovers(Object result) {
        if (result instanceof Cover) {
            return List.of((Cover) result);
        } else if (result instanceof List) {
            return (List<Cover>) result;
        }
        return List.of();
    }

    @SuppressWarnings("unchecked")
    private List<CommandBook> packResult(Object result, String correlationId) {
        if (result instanceof CommandBook) {
            return List.of((CommandBook) result);
        } else if (result instanceof List) {
            return (List<CommandBook>) result;
        }
        return List.of();
    }
}
