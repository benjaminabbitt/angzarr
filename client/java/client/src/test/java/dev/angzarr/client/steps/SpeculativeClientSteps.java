package dev.angzarr.client.steps;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Empty;
import dev.angzarr.BusinessResponse;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.Cover;
import dev.angzarr.UUID;
import dev.angzarr.client.Errors;
import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for speculative client scenarios.
 */
public class SpeculativeClientSteps {

    private EventBook eventBook;
    private BusinessResponse response;
    private Exception error;
    private int baseEventCount;

    @Before
    public void setup() {
        eventBook = null;
        response = null;
        error = null;
        baseEventCount = 0;
    }

    // ==========================================================================
    // Speculative PM Steps
    // ==========================================================================

    @Then("the speculative PM operation should fail")
    public void theSpeculativePMOperationShouldFail() {
        assertThat(error).isNotNull();
    }

    @Then("the speculative operation should fail with connection error")
    public void theSpeculativeOperationShouldFailWithConnectionError() {
        assertThat(error).isNotNull();
        boolean isConnectionError = error instanceof Errors.ConnectionError
            || error.getMessage().toLowerCase().contains("connection");
        assertThat(isConnectionError).isTrue();
    }

    @Then("the speculative operation should fail with invalid argument error")
    public void theSpeculativeOperationShouldFailWithInvalidArgumentError() {
        assertThat(error).isNotNull();
        assertThat(error).isInstanceOf(Errors.InvalidArgumentError.class);
    }

    // ==========================================================================
    // Speculative Aggregate Steps
    // ==========================================================================

    @Given("a speculative aggregate {string} with root {string} has {int} events")
    public void aSpeculativeAggregateWithRootHasEvents(String domain, String root, int count) {
        UUID rootUuid = parseRoot(root);
        EventBook.Builder builder = EventBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain(domain)
                .setRoot(rootUuid));

        for (int i = 0; i < count; i++) {
            builder.addPages(EventPage.newBuilder()
                .setSequence(i)
                .setEvent(Any.pack(Empty.getDefaultInstance())));
        }

        eventBook = builder.build();
        baseEventCount = count;
    }

    @When("I verify the real events for {string} root {string}")
    public void iVerifyTheRealEventsForRoot(String domain, String root) {
        // Verify the real (non-speculative) events match base count
        response = BusinessResponse.newBuilder()
            .setEvents(eventBook)
            .build();
    }

    // ==========================================================================
    // Helper Methods
    // ==========================================================================

    private UUID parseRoot(String input) {
        try {
            java.util.UUID uuid = java.util.UUID.fromString(input);
            return UUID.newBuilder()
                .setValue(ByteString.copyFrom(uuidToBytes(uuid)))
                .build();
        } catch (IllegalArgumentException e) {
            // Not a valid UUID, hash the string
            try {
                MessageDigest md = MessageDigest.getInstance("MD5");
                byte[] hash = md.digest(input.getBytes(StandardCharsets.UTF_8));
                return UUID.newBuilder()
                    .setValue(ByteString.copyFrom(hash))
                    .build();
            } catch (NoSuchAlgorithmException ex) {
                throw new RuntimeException(ex);
            }
        }
    }

    private byte[] uuidToBytes(java.util.UUID uuid) {
        byte[] bytes = new byte[16];
        long msb = uuid.getMostSignificantBits();
        long lsb = uuid.getLeastSignificantBits();
        for (int i = 0; i < 8; i++) {
            bytes[i] = (byte) ((msb >> (56 - i * 8)) & 0xFF);
            bytes[i + 8] = (byte) ((lsb >> (56 - i * 8)) & 0xFF);
        }
        return bytes;
    }
}
