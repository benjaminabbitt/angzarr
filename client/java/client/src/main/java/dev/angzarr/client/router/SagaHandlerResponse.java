package dev.angzarr.client.router;

import dev.angzarr.CommandBook;
import dev.angzarr.EventBook;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Response from saga handlers.
 *
 * <p>Sagas produce commands (to send to other aggregates) and optionally
 * events/facts (to inject directly into target aggregates).
 */
public class SagaHandlerResponse {

    private final List<CommandBook> commands;
    private final List<EventBook> events;

    private SagaHandlerResponse(List<CommandBook> commands, List<EventBook> events) {
        this.commands = commands != null ? commands : Collections.emptyList();
        this.events = events != null ? events : Collections.emptyList();
    }

    /**
     * Create an empty response (no commands, no events).
     */
    public static SagaHandlerResponse empty() {
        return new SagaHandlerResponse(Collections.emptyList(), Collections.emptyList());
    }

    /**
     * Create a response with commands only.
     */
    public static SagaHandlerResponse withCommands(List<CommandBook> commands) {
        return new SagaHandlerResponse(new ArrayList<>(commands), Collections.emptyList());
    }

    /**
     * Create a response with events only.
     */
    public static SagaHandlerResponse withEvents(List<EventBook> events) {
        return new SagaHandlerResponse(Collections.emptyList(), new ArrayList<>(events));
    }

    /**
     * Create a response with both commands and events.
     */
    public static SagaHandlerResponse withBoth(List<CommandBook> commands, List<EventBook> events) {
        return new SagaHandlerResponse(new ArrayList<>(commands), new ArrayList<>(events));
    }

    /**
     * Get the commands to send to other aggregates.
     */
    public List<CommandBook> getCommands() {
        return commands;
    }

    /**
     * Check if response has commands.
     */
    public boolean hasCommands() {
        return !commands.isEmpty();
    }

    /**
     * Get the events to inject into target aggregates.
     */
    public List<EventBook> getEvents() {
        return events;
    }

    /**
     * Check if response has events.
     */
    public boolean hasEvents() {
        return !events.isEmpty();
    }
}
