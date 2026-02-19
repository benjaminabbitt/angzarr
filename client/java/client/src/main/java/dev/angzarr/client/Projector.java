package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.client.annotations.Projects;

import java.lang.reflect.Method;
import java.util.*;
import java.util.function.Function;

/**
 * Base class for projectors using annotation-based handler registration.
 *
 * <p>Usage (single domain):
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
 *
 * <p>Usage (multi-domain):
 * <pre>{@code
 * public class OutputProjector extends Projector {
 *     public OutputProjector() {
 *         super("output", List.of("player", "table", "hand"));
 *     }
 *
 *     @Projects(PlayerRegistered.class)
 *     public Projection projectRegistered(PlayerRegistered event) {
 *         writeLog("PLAYER registered: " + event.getDisplayName());
 *         return Projection.upsert("log", "...");
 *     }
 * }
 * }</pre>
 */
public abstract class Projector {
    private final String name;
    private final List<String> inputDomains;
    private final Map<String, Function<Any, Projection>> handlers = new HashMap<>();

    /**
     * Constructor for single-domain projectors.
     */
    protected Projector(String name, String inputDomain) {
        this.name = name;
        this.inputDomains = List.of(inputDomain);
        buildDispatchTable();
    }

    /**
     * Constructor for multi-domain projectors.
     */
    protected Projector(String name, List<String> inputDomains) {
        this.name = name;
        this.inputDomains = inputDomains;
        buildDispatchTable();
    }

    public String getName() {
        return name;
    }

    /**
     * Get the input domain (first domain for multi-domain projectors).
     */
    public String getInputDomain() {
        return inputDomains.isEmpty() ? "" : inputDomains.get(0);
    }

    /**
     * Get all input domains.
     */
    public List<String> getInputDomains() {
        return inputDomains;
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
        List<TargetDesc> inputs = new ArrayList<>();
        for (String domain : inputDomains) {
            inputs.add(new TargetDesc(domain, types));
        }
        return new Descriptor(
            name,
            ComponentTypes.PROJECTOR,
            inputs
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
