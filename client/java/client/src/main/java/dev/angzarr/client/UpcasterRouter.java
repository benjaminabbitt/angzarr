package dev.angzarr.client;

import com.google.protobuf.Any;
import dev.angzarr.EventPage;

import java.util.ArrayList;
import java.util.List;
import java.util.function.Function;

/**
 * Event version transformer.
 *
 * <p>Matches old event type_url suffixes and transforms to new versions.
 * Events without registered transformations pass through unchanged.
 *
 * <p>Example:
 * <pre>{@code
 * UpcasterRouter router = new UpcasterRouter("order")
 *     .on("OrderCreatedV1", old -> {
 *         OrderCreatedV1 v1 = old.unpack(OrderCreatedV1.class);
 *         return Any.pack(OrderCreated.newBuilder()
 *             .setOrderId(v1.getOrderId())
 *             .build());
 *     });
 *
 * List<EventPage> newEvents = router.upcast(oldEvents);
 * }</pre>
 */
public class UpcasterRouter {
    private final String domain;
    private final List<UpcasterEntry> handlers;

    private static class UpcasterEntry {
        final String suffix;
        final Function<Any, Any> handler;

        UpcasterEntry(String suffix, Function<Any, Any> handler) {
            this.suffix = suffix;
            this.handler = handler;
        }
    }

    /**
     * Create a new upcaster router for a domain.
     *
     * @param domain The domain this upcaster handles
     */
    public UpcasterRouter(String domain) {
        this.domain = domain;
        this.handlers = new ArrayList<>();
    }

    /**
     * Register a handler for an old event type_url suffix.
     *
     * <p>The suffix is matched against the end of the event's type_url.
     * For example, suffix "OrderCreatedV1" matches
     * "type.googleapis.com/examples.OrderCreatedV1".
     *
     * @param suffix The type_url suffix to match
     * @param handler Function that transforms old event to new event
     * @return this router for fluent chaining
     */
    public UpcasterRouter on(String suffix, Function<Any, Any> handler) {
        handlers.add(new UpcasterEntry(suffix, handler));
        return this;
    }

    /**
     * Transform a list of events to current versions.
     *
     * <p>Events matching registered handlers are transformed.
     * Events without matching handlers pass through unchanged.
     *
     * @param events List of EventPages to transform
     * @return List of EventPages with transformed events
     */
    public List<EventPage> upcast(List<EventPage> events) {
        List<EventPage> result = new ArrayList<>(events.size());

        for (EventPage page : events) {
            if (!page.hasEvent()) {
                result.add(page);
                continue;
            }

            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();
            boolean transformed = false;

            for (UpcasterEntry entry : handlers) {
                if (typeUrl.endsWith(entry.suffix)) {
                    Any newEvent = entry.handler.apply(event);
                    EventPage newPage = EventPage.newBuilder()
                            .setEvent(newEvent)
                            .setSequence(page.getSequence())
                            .setCreatedAt(page.getCreatedAt())
                            .build();
                    result.add(newPage);
                    transformed = true;
                    break;
                }
            }

            if (!transformed) {
                result.add(page);
            }
        }

        return result;
    }

    /**
     * Get the domain this upcaster handles.
     *
     * @return The domain name
     */
    public String getDomain() {
        return domain;
    }
}
