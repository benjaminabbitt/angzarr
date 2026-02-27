package dev.angzarr.client.router;

import dev.angzarr.CommandBook;
import dev.angzarr.EventBook;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Response from process manager handlers.
 *
 * <p>Process managers produce:
 * <ul>
 *   <li>Commands (to send to other aggregates)</li>
 *   <li>Process events (to persist their own state)</li>
 *   <li>Facts (events to inject directly into other aggregates)</li>
 * </ul>
 */
public class ProcessManagerResponse {

    private final List<CommandBook> commands;
    private final EventBook processEvents;
    private final List<EventBook> facts;

    private ProcessManagerResponse(List<CommandBook> commands, EventBook processEvents, List<EventBook> facts) {
        this.commands = commands != null ? commands : Collections.emptyList();
        this.processEvents = processEvents;
        this.facts = facts != null ? facts : Collections.emptyList();
    }

    /**
     * Create an empty response (no commands, no process events, no facts).
     */
    public static ProcessManagerResponse empty() {
        return new ProcessManagerResponse(Collections.emptyList(), null, Collections.emptyList());
    }

    /**
     * Create a response with commands only.
     */
    public static ProcessManagerResponse withCommands(List<CommandBook> commands) {
        return new ProcessManagerResponse(new ArrayList<>(commands), null, Collections.emptyList());
    }

    /**
     * Create a response with process events only.
     */
    public static ProcessManagerResponse withProcessEvents(EventBook processEvents) {
        return new ProcessManagerResponse(Collections.emptyList(), processEvents, Collections.emptyList());
    }

    /**
     * Create a response with facts only.
     */
    public static ProcessManagerResponse withFacts(List<EventBook> facts) {
        return new ProcessManagerResponse(Collections.emptyList(), null, new ArrayList<>(facts));
    }

    /**
     * Create a response with commands and process events.
     */
    public static ProcessManagerResponse withBoth(List<CommandBook> commands, EventBook processEvents) {
        return new ProcessManagerResponse(new ArrayList<>(commands), processEvents, Collections.emptyList());
    }

    /**
     * Create a response with all fields.
     */
    public static ProcessManagerResponse withAll(List<CommandBook> commands, EventBook processEvents, List<EventBook> facts) {
        return new ProcessManagerResponse(
            new ArrayList<>(commands),
            processEvents,
            new ArrayList<>(facts)
        );
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
     * Get the process events to persist to PM's own domain.
     */
    public EventBook getProcessEvents() {
        return processEvents;
    }

    /**
     * Check if response has process events.
     */
    public boolean hasProcessEvents() {
        return processEvents != null;
    }

    /**
     * Get the facts to inject into other aggregates.
     */
    public List<EventBook> getFacts() {
        return facts;
    }

    /**
     * Check if response has facts.
     */
    public boolean hasFacts() {
        return !facts.isEmpty();
    }
}
