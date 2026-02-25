package dev.angzarr.client.router;

import org.junit.jupiter.api.Test;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Tests for CommandRejectedError.
 *
 * Verifies the error type for command/event rejection with human-readable reasons.
 */
class CommandRejectedErrorTest {

    @Test
    void constructor_sets_reason() {
        CommandRejectedError error = new CommandRejectedError("insufficient funds");

        assertThat(error.getReason()).isEqualTo("insufficient funds");
    }

    @Test
    void message_includes_reason() {
        CommandRejectedError error = new CommandRejectedError("invalid input");

        assertThat(error.getMessage()).isEqualTo("Command rejected: invalid input");
    }

    @Test
    void static_factory_creates_error() {
        CommandRejectedError error = CommandRejectedError.of("player does not exist");

        assertThat(error.getReason()).isEqualTo("player does not exist");
        assertThat(error.getMessage()).isEqualTo("Command rejected: player does not exist");
    }

    @Test
    void constructor_with_cause_preserves_cause() {
        RuntimeException cause = new RuntimeException("underlying error");
        CommandRejectedError error = new CommandRejectedError("operation failed", cause);

        assertThat(error.getReason()).isEqualTo("operation failed");
        assertThat(error.getCause()).isEqualTo(cause);
    }
}
