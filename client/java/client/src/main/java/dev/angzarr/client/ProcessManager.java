package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.client.annotations.Applies;
import dev.angzarr.client.annotations.ReactsTo;
import dev.angzarr.client.annotations.Rejected;

import java.lang.reflect.Method;
import java.util.*;
import java.util.function.BiConsumer;
import java.util.function.BiFunction;

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
    private final Map<String, BiFunction<Notification, S, EventBook>> rejectionHandlers = new HashMap<>();

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
     * Dispatch events and produce commands.
     */
    public List<CommandBook> dispatch(EventBook book, EventBook priorEvents) {
        rebuildState(priorEvents);

        String correlationId = book.hasCover() ? book.getCover().getCorrelationId() : "";
        if (correlationId.isEmpty()) {
            // PMs require correlation ID
            return List.of();
        }

        List<CommandBook> commands = new ArrayList<>();
        for (EventPage page : book.getPagesList()) {
            if (!page.hasEvent()) continue;

            // Check for rejection notification
            if (Helpers.typeUrlMatches(page.getEvent().getTypeUrl(), "Notification")) {
                try {
                    Notification notification = page.getEvent().unpack(Notification.class);
                    dispatchRejection(notification);
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
        return commands;
    }

    /**
     * Build a component descriptor.
     */
    public Descriptor getDescriptor() {
        List<String> types = new ArrayList<>(handlers.keySet());
        return new Descriptor(
            name,
            ComponentTypes.PROCESS_MANAGER,
            List.of() // PM subscribes to multiple domains
        );
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

    private void dispatchRejection(Notification notification) {
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
        BiFunction<Notification, S, EventBook> handler = rejectionHandlers.get(key);
        if (handler != null) {
            handler.apply(notification, state);
        }
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
                        return (EventBook) method.invoke(this, notification);
                    } catch (Exception e) {
                        throw new RuntimeException("Failed to handle rejection for " + key, e);
                    }
                });
            }
        }
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
