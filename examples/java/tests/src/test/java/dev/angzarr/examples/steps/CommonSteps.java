package dev.angzarr.examples.steps;

import dev.angzarr.client.Errors;
import io.cucumber.java.en.Then;
import io.grpc.Status;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Shared step definitions used across all aggregate tests.
 *
 * Aggregate-specific step classes should set lastRejectedError
 * when their command handling catches an error.
 */
public class CommonSteps {

    // Shared error state - set by aggregate step classes
    private static Errors.CommandRejectedError lastRejectedError;

    /**
     * Set the last rejected error (called by aggregate step classes).
     */
    public static void setLastRejectedError(Errors.CommandRejectedError error) {
        lastRejectedError = error;
    }

    /**
     * Clear the last rejected error (called at start of scenarios).
     */
    public static void clearLastRejectedError() {
        lastRejectedError = null;
    }

    /**
     * Get the last rejected error.
     */
    public static Errors.CommandRejectedError getLastRejectedError() {
        return lastRejectedError;
    }

    @Then("the command fails with status {string}")
    public void commandFailsWithStatus(String status) {
        assertThat(lastRejectedError)
            .withFailMessage("Expected command to fail but it succeeded")
            .isNotNull();
        Status.Code expectedCode = Status.Code.valueOf(status);
        assertThat(lastRejectedError.getStatusCode()).isEqualTo(expectedCode);
    }

    @Then("the error message contains {string}")
    public void errorMessageContains(String substring) {
        assertThat(lastRejectedError)
            .withFailMessage("Expected command to fail but it succeeded")
            .isNotNull();
        assertThat(lastRejectedError.getMessage().toLowerCase())
            .contains(substring.toLowerCase());
    }
}
