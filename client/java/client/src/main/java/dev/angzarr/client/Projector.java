package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.client.annotations.Projects;

import java.lang.reflect.Method;
import java.util.*;
import java.util.function.Function;

/**
 * Projection result from a projector handler.
 */
class Projection {
    private final String key;
    private final String value;
    private final boolean isDelete;

    private Projection(String key, String value, boolean isDelete) {
        this.key = key;
        this.value = value;
        this.isDelete = isDelete;
    }

    public static Projection upsert(String key, String value) {
        return new Projection(key, value, false);
    }

    public static Projection delete(String key) {
        return new Projection(key, "", true);
    }

    public String getKey() {
        return key;
    }

    public String getValue() {
        return value;
    }

    public boolean isDelete() {
        return isDelete;
    }
}

/**
 * Base class for projectors using annotation-based handler registration.
 *
 * <p>Usage:
 * <pre>{@code
 * public class StockProjector extends Projector {
 *     public StockProjector() {
 *         super("projector-inventory-stock", "inventory");
 *     }
 *
 *     @Projects(StockInitialized.class)
 *     public Projection projectStockInitialized(StockInitialized event) {
 *         return Projection.upsert(event.getSku(), String.valueOf(event.getQuantity()));
 *     }
 * }
 * }</pre>
 */
public abstract class Projector {
    private final String name;
    private final String inputDomain;
    private final Map<String, Function<Any, Projection>> handlers = new HashMap<>();

    protected Projector(String name, String inputDomain) {
        this.name = name;
        this.inputDomain = inputDomain;
        buildDispatchTable();
    }

    public String getName() {
        return name;
    }

    public String getInputDomain() {
        return inputDomain;
    }

    /**
     * Project all events in the book.
     */
    public List<Projection> project(EventBook book) {
        List<Projection> projections = new ArrayList<>();

        for (EventPage page : book.getPagesList()) {
            if (!page.hasEvent()) continue;

            String suffix = Helpers.typeNameFromUrl(page.getEvent().getTypeUrl());
            Function<Any, Projection> handler = handlers.get(suffix);
            if (handler != null) {
                projections.add(handler.apply(page.getEvent()));
            }
        }
        return projections;
    }

    /**
     * Build a component descriptor.
     */
    public Descriptor getDescriptor() {
        List<String> types = new ArrayList<>(handlers.keySet());
        return new Descriptor(
            name,
            ComponentTypes.PROJECTOR,
            List.of(new TargetDesc(inputDomain, types))
        );
    }

    private void buildDispatchTable() {
        for (Method method : this.getClass().getDeclaredMethods()) {
            Projects projects = method.getAnnotation(Projects.class);
            if (projects != null) {
                Class<? extends Message> eventType = projects.value();
                String suffix = eventType.getSimpleName();
                method.setAccessible(true);

                handlers.put(suffix, (eventAny) -> {
                    try {
                        Message event = eventAny.unpack(eventType);
                        return (Projection) method.invoke(this, event);
                    } catch (Exception e) {
                        throw new RuntimeException("Failed to project event " + suffix, e);
                    }
                });
            }
        }
    }
}
