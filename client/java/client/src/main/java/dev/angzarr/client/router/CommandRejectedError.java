package dev.angzarr.client.router;

/**
 * Error type for command/event rejection with a human-readable reason.
 *
 * <p>This is a checked exception used in command handlers to indicate
 * that a command was rejected due to business rule violations.
 */
public class CommandRejectedError extends Exception {

    private final String reason;

    public CommandRejectedError(String reason) {
        super("Command rejected: " + reason);
        this.reason = reason;
    }

    public CommandRejectedError(String reason, Throwable cause) {
        super("Command rejected: " + reason, cause);
        this.reason = reason;
    }

    /**
     * Get the rejection reason.
     */
    public String getReason() {
        return reason;
    }

    /**
     * Create a new CommandRejectedError with the given reason.
     */
    public static CommandRejectedError of(String reason) {
        return new CommandRejectedError(reason);
    }
}
