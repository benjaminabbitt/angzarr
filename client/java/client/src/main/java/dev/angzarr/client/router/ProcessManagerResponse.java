package dev.angzarr.client.router;

import dev.angzarr.CommandBook;
import dev.angzarr.EventBook;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;

/**
 * Response from process manager handlers.
 *
 * <p>Process managers produce both commands (to send to other aggregates)
 * and process events (to persist their own state).
 */
public class ProcessManagerResponse {

    private final List<CommandBook> commands;
    private final EventBook processEvents;

    private ProcessManagerResponse(List<CommandBook> commands, EventBook processEvents) {
        this.commands = commands != null ? commands : Collections.emptyList();
        this.processEvents = processEvents;
    }

    /**
     * Create an empty response (no commands, no process events).
     */
    public static ProcessManagerResponse empty() {
        return new ProcessManagerResponse(Collections.emptyList(), null);
    }

    /**
     * Create a response with commands only.
     */
    public static ProcessManagerResponse withCommands(List<CommandBook> commands) {
        return new ProcessManagerResponse(new ArrayList<>(commands), null);
    }

    /**
     * Create a response with process events only.
     */
    public static ProcessManagerResponse withProcessEvents(EventBook processEvents) {
        return new ProcessManagerResponse(Collections.emptyList(), processEvents);
    }

    /**
     * Create a response with both commands and process events.
     */
    public static ProcessManagerResponse withBoth(List<CommandBook> commands, EventBook processEvents) {
        return new ProcessManagerResponse(new ArrayList<>(commands), processEvents);
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
}
