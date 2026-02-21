package dev.angzarr.client;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;

import java.util.*;
import java.util.function.BiFunction;

/**
 * Fluent state reconstruction from events (functional pattern).
 */
public class StateRouter<S> {
    private final java.util.function.Supplier<S> factory;
    private final Map<String, BiFunction<S, Any, Void>> appliers = new HashMap<>();

    public StateRouter(java.util.function.Supplier<S> factory) {
        this.factory = factory;
    }

    public <E extends Message> StateRouter<S> on(Class<E> eventType, java.util.function.BiConsumer<S, E> applier) {
        var suffix = eventType.getSimpleName();
        appliers.put(suffix, (state, any) -> {
            try {
                @SuppressWarnings("unchecked")
                E event = (E) any.unpack(eventType);
                applier.accept(state, event);
            } catch (InvalidProtocolBufferException ignored) {}
            return null;
        });
        return this;
    }

    public S withEventBook(EventBook book) {
        var state = factory.get();
        if (book == null) return state;

        for (var page : book.getPagesList()) {
            if (!page.hasEvent()) continue;
            applyEvent(state, page.getEvent());
        }
        return state;
    }

    private void applyEvent(S state, Any eventAny) {
        for (var entry : appliers.entrySet()) {
            if (eventAny.getTypeUrl().endsWith(entry.getKey())) {
                entry.getValue().apply(state, eventAny);
                return;
            }
        }
    }
}
