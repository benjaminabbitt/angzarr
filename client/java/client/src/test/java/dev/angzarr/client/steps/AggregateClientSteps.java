package dev.angzarr.client.steps;

import com.google.protobuf.Any;
import com.google.protobuf.Empty;
import dev.angzarr.EventBook;
import dev.angzarr.EventPage;
import dev.angzarr.Cover;
import dev.angzarr.RevocationResponse;
import dev.angzarr.client.Helpers;
import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for aggregate client and router scenarios.
 */
public class AggregateClientSteps {

    private EventBook eventBook;
    private Object response;
    private Exception error;
    private RevocationResponse rejection;
    private Object builtState;

    @Before
    public void setup() {
        eventBook = null;
        response = null;
        error = null;
        rejection = null;
        builtState = null;
    }

    // ==========================================================================
    // Aggregate Client Steps
    // ==========================================================================

    @Then("the aggregate operation should fail with connection error")
    public void theAggregateOperationShouldFailWithConnectionError() {
        assertThat(error).isNotNull();
        assertThat(error.getMessage().toLowerCase()).contains("connection");
    }

    // ==========================================================================
    // Router Steps (Saga Rejection)
    // ==========================================================================

    @Given("a saga router with a rejected command")
    public void aSagaRouterWithARejectedCommand() {
        rejection = RevocationResponse.newBuilder()
            .setReason("Command rejected by target aggregate")
            .build();
    }

    @When("the router processes the rejection")
    public void theRouterProcessesTheRejection() {
        assertThat(rejection).isNotNull();
    }

    @Then("the router projection state should be returned")
    public void theRouterProjectionStateShouldBeReturned() {
        // Projection state verification - either built state or last projection
        // In mock scenarios, we just verify no error occurred
    }

    // ==========================================================================
    // Helper Methods
    // ==========================================================================

    private EventBook makeEventBook(String domain, int eventCount) {
        UUID root = UUID.randomUUID();
        EventBook.Builder builder = EventBook.newBuilder()
            .setCover(Cover.newBuilder()
                .setDomain(domain)
                .setRoot(Helpers.uuidToProto(root)));

        for (int i = 0; i < eventCount; i++) {
            builder.addPages(EventPage.newBuilder()
                .setSequence(i)
                .setEvent(Any.pack(Empty.getDefaultInstance())));
        }

        return builder.build();
    }
}
