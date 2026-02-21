package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.client.annotations.Prepares;
import dev.angzarr.client.annotations.ReactsTo;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.util.*;
import java.util.function.BiFunction;
import java.util.function.Function;

/**
 * Base class for sagas using annotation-based handler registration.
 *
 * <p>Usage:
 * <pre>{@code
 * public class OrderFulfillmentSaga extends Saga {
 *     public OrderFulfillmentSaga() {
 *         super("saga-order-fulfillment", "order", "fulfillment");
 *     }
 *
 *     @Prepares(OrderCompleted.class)
 *     public List<Cover> prepareOrderCompleted(OrderCompleted event) {
 *         return List.of(Cover.newBuilder()
 *             .setDomain("fulfillment")
 *             .setRoot(UUID.newBuilder().setValue(event.getFulfillmentId()))
 *             .build());
 *     }
 *
 *     @ReactsTo(OrderCompleted.class)
 *     public CreateShipment handleOrderCompleted(OrderCompleted event) {
 *         return CreateShipment.newBuilder()
 *             .setOrderId(event.getOrderId())
 *             .build();
 *     }
 * }
 * }</pre>
 */
public abstract class Saga {
    private final String name;
    private final String inputDomain;
    private final String outputDomain;
    private final Map<String, BiFunction<Any, String, List<CommandBook>>> handlers = new HashMap<>();
    private final Map<String, Function<Any, List<Cover>>> prepareHandlers = new HashMap<>();

    protected Saga(String name, String inputDomain, String outputDomain) {
        this.name = name;
        this.inputDomain = inputDomain;
        this.outputDomain = outputDomain;
        buildDispatchTables();
    }

    public String getName() {
        return name;
    }

    public String getInputDomain() {
        return inputDomain;
    }

    public String getOutputDomain() {
        return outputDomain;
    }

    /**
     * Get destinations needed for source events (two-phase protocol).
     */
    public List<Cover> prepareDestinations(EventBook book) {
        List<Cover> destinations = new ArrayList<>();

        for (EventPage page : book.getPagesList()) {
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
     * Dispatch all events to handlers.
     */
    public List<CommandBook> dispatch(EventBook book, List<EventBook> destinations) {
        String correlationId = book.hasCover() ? book.getCover().getCorrelationId() : "";

        List<CommandBook> commands = new ArrayList<>();
        for (EventPage page : book.getPagesList()) {
            if (!page.hasEvent()) continue;

            String suffix = Helpers.typeNameFromUrl(page.getEvent().getTypeUrl());
            BiFunction<Any, String, List<CommandBook>> handler = handlers.get(suffix);
            if (handler != null) {
                commands.addAll(handler.apply(page.getEvent(), correlationId));
            }
        }
        return commands;
    }

    /**
     * Pack a command into a CommandBook.
     */
    protected List<CommandBook> packCommands(Message command, String correlationId) {
        CommandBook.Builder builder = CommandBook.newBuilder();
        Cover.Builder cover = Cover.newBuilder()
            .setDomain(outputDomain)
            .setCorrelationId(correlationId);
        builder.setCover(cover);

        CommandPage.Builder page = CommandPage.newBuilder();
        page.setCommand(Any.pack(command, "type.googleapis.com/"));
        builder.addPages(page);

        return List.of(builder.build());
    }

    /**
     * Pack multiple commands into CommandBooks.
     */
    protected List<CommandBook> packCommands(List<? extends Message> commands, String correlationId) {
        List<CommandBook> books = new ArrayList<>();
        for (Message cmd : commands) {
            books.addAll(packCommands(cmd, correlationId));
        }
        return books;
    }

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
    private List<CommandBook> packResult(Object result, String correlationId) {
        if (result instanceof Message) {
            return packCommands((Message) result, correlationId);
        } else if (result instanceof List) {
            return packCommands((List<? extends Message>) result, correlationId);
        }
        return List.of();
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

    /**
     * Calculate next sequence number from an EventBook.
     */
    public static int nextSequence(EventBook eventBook) {
        return Helpers.nextSequence(eventBook);
    }
}
