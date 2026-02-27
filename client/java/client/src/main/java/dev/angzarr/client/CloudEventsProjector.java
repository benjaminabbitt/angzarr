package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.client.annotations.Publishes;

import java.lang.reflect.Method;
import java.util.*;
import java.util.function.Function;

/**
 * Base class for CloudEvents projectors using annotation-based handler registration.
 *
 * <p>CloudEvents projectors transform internal domain events into CloudEvents 1.0 format
 * for external consumption via HTTP webhooks or Kafka.
 *
 * <p>Usage:
 * <pre>{@code
 * public class PlayerCloudEventsProjector extends CloudEventsProjector {
 *     public PlayerCloudEventsProjector() {
 *         super("prj-player-cloudevents", "player");
 *     }
 *
 *     @Publishes("PlayerRegistered")
 *     public CloudEvent onPlayerRegistered(PlayerRegistered event) {
 *         var publicEvent = PublicPlayerRegistered.newBuilder()
 *             .setDisplayName(event.getDisplayName())
 *             .build();
 *         return CloudEvent.newBuilder()
 *             .setType("com.poker.player.registered")
 *             .setData(Any.pack(publicEvent))
 *             .build();
 *     }
 * }
 * }</pre>
 */
public abstract class CloudEventsProjector {
    private final String name;
    private final String inputDomain;
    private final Map<String, Handler> handlers = new HashMap<>();

    private record Handler(Class<? extends Message> eventType, Method method) {}

    protected CloudEventsProjector(String name, String inputDomain) {
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
     * Process all events in the book and return CloudEvents.
     */
    public List<CloudEvent> project(EventBook book) {
        List<CloudEvent> events = new ArrayList<>();

        for (EventPage page : book.getPagesList()) {
            if (!page.hasEvent()) continue;

            String suffix = Helpers.typeNameFromUrl(page.getEvent().getTypeUrl());
            Handler handler = handlers.get(suffix);
            if (handler != null) {
                CloudEvent result = dispatch(handler, page.getEvent());
                if (result != null) {
                    events.add(result);
                }
            }
        }
        return events;
    }

    private CloudEvent dispatch(Handler handler, Any eventAny) {
        try {
            Message event = eventAny.unpack(handler.eventType());
            return (CloudEvent) handler.method().invoke(this, event);
        } catch (Exception e) {
            throw new RuntimeException("Failed to project event", e);
        }
    }

    private void buildDispatchTable() {
        for (Method method : this.getClass().getDeclaredMethods()) {
            Publishes publishes = method.getAnnotation(Publishes.class);
            if (publishes != null) {
                String suffix = publishes.value();
                Class<?>[] paramTypes = method.getParameterTypes();
                if (paramTypes.length == 1 && Message.class.isAssignableFrom(paramTypes[0])) {
                    @SuppressWarnings("unchecked")
                    Class<? extends Message> eventType = (Class<? extends Message>) paramTypes[0];
                    method.setAccessible(true);
                    handlers.put(suffix, new Handler(eventType, method));
                }
            }
        }
    }
}
