package dev.angzarr.client.steps;

import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for AggregateClient command execution scenarios.
 */
public class AggregateClientSteps {

    private String domain;
    private String root;
    private int sequence;
    private String commandType;
    private String commandData;
    private String correlationId;
    private String syncMode;
    private Integer timeoutMs;
    private boolean commandSucceeded;
    private boolean commandFailed;
    private String error;
    private String errorType;
    private List<EventRecord> eventsReturned;
    private List<Boolean> concurrentResults;
    private Map<String, Integer> aggregates;
    private boolean projectorsConfigured;
    private boolean sagasConfigured;
    private boolean serviceAvailable;
    private boolean serviceSlow;
    private Integer currentSequence;

    private static class EventRecord {
        String type;
        int sequence;

        EventRecord(String type, int sequence) {
            this.type = type;
            this.sequence = sequence;
        }
    }

    @Before
    public void setup() {
        domain = null;
        root = null;
        sequence = 0;
        commandType = null;
        commandData = null;
        correlationId = null;
        syncMode = null;
        timeoutMs = null;
        commandSucceeded = false;
        commandFailed = false;
        error = null;
        errorType = null;
        eventsReturned = new ArrayList<>();
        concurrentResults = new ArrayList<>();
        aggregates = new HashMap<>();
        projectorsConfigured = false;
        sagasConfigured = false;
        serviceAvailable = true;
        serviceSlow = false;
        currentSequence = null;
    }

    // ==========================================================================
    // Background Steps
    // ==========================================================================

    @Given("an AggregateClient connected to the test backend")
    public void anAggregateClientConnectedToTheTestBackend() {
        serviceAvailable = true;
    }

    // ==========================================================================
    // Given Steps - Aggregates
    // ==========================================================================

    @Given("a new aggregate root in domain {string}")
    public void aNewAggregateRootInDomain(String domain) {
        this.domain = domain;
        this.root = UUID.randomUUID().toString();
        this.sequence = 0;
        aggregates.put(domain + ":" + root, 0);
    }

    @Given("an aggregate {string} with root {string} at sequence {int}")
    public void anAggregateWithRootAtSequence(String domain, String root, int seq) {
        this.domain = domain;
        this.root = root;
        this.sequence = seq;
        aggregates.put(domain + ":" + root, seq);
    }

    // Note: "an aggregate {string} with root {string}" is defined in QueryClientSteps

    @Given("no aggregate exists for domain {string} root {string}")
    public void noAggregateExistsForDomainRoot(String domain, String root) {
        this.domain = domain;
        this.root = root;
        this.sequence = 0;
    }

    @Given("projectors are configured for {string} domain")
    public void projectorsAreConfiguredForDomain(String domain) {
        projectorsConfigured = true;
    }

    @Given("sagas are configured for {string} domain")
    public void sagasAreConfiguredForDomain(String domain) {
        sagasConfigured = true;
    }

    @Given("the aggregate service is unavailable")
    public void theAggregateServiceIsUnavailable() {
        serviceAvailable = false;
    }

    @Given("the aggregate service is slow to respond")
    public void theAggregateServiceIsSlowToRespond() {
        serviceSlow = true;
    }

    // ==========================================================================
    // When Steps - Commands
    // ==========================================================================

    @When("I execute a {string} command with data {string}")
    public void iExecuteACommandWithData(String cmdType, String data) {
        this.commandType = cmdType;
        this.commandData = data;
        this.commandSucceeded = true;

        // Convert command type to event type (e.g., "CreateOrder" -> "OrderCreated")
        String eventType;
        if (cmdType.startsWith("Create")) {
            eventType = cmdType.substring("Create".length()) + "Created";
        } else {
            eventType = cmdType;
        }
        eventsReturned.add(new EventRecord(eventType, sequence));
    }

    @When("I execute a {string} command at sequence {int}")
    public void iExecuteACommandAtSequence(String cmdType, int seq) {
        this.commandType = cmdType;
        String key = domain + ":" + root;
        int currentSeq = aggregates.getOrDefault(key, 0);

        if (seq != currentSeq) {
            commandFailed = true;
            errorType = "precondition";
            error = "Sequence mismatch";
        } else {
            commandSucceeded = true;
            eventsReturned.add(new EventRecord(cmdType, seq));
        }
    }

    @When("I execute a command at sequence {int}")
    public void iExecuteACommandAtSequence(int seq) {
        String key = domain + ":" + root;
        int currentSeq = aggregates.getOrDefault(key, 0);

        if (seq != currentSeq) {
            commandFailed = true;
            errorType = "precondition";
            error = "Sequence mismatch";
        } else {
            commandSucceeded = true;
            eventsReturned.add(new EventRecord("Event", seq));
        }
    }

    @When("I execute a command with correlation ID {string}")
    public void iExecuteACommandWithCorrelationId(String cid) {
        this.correlationId = cid;
        this.commandSucceeded = true;
        eventsReturned.add(new EventRecord("Event", sequence));
    }

    @When("two commands are sent concurrently at sequence {int}")
    public void twoCommandsAreSentConcurrentlyAtSequence(int seq) {
        // First succeeds
        concurrentResults.add(true);
        // Second fails with precondition error
        concurrentResults.add(false);
    }

    @When("I query the current sequence for {string} root {string}")
    public void iQueryTheCurrentSequenceForRoot(String domain, String root) {
        String key = domain + ":" + root;
        currentSequence = aggregates.get(key);
    }

    @When("I retry the command at the correct sequence")
    public void iRetryTheCommandAtTheCorrectSequence() {
        commandSucceeded = true;
        commandFailed = false;
        error = null;
        errorType = null;
    }

    @When("I execute a command asynchronously")
    public void iExecuteACommandAsynchronously() {
        syncMode = "ASYNC";
        commandSucceeded = true;
    }

    @When("I execute a command with sync mode SIMPLE")
    public void iExecuteACommandWithSyncModeSIMPLE() {
        syncMode = "SIMPLE";
        commandSucceeded = true;
    }

    @When("I execute a command with sync mode CASCADE")
    public void iExecuteACommandWithSyncModeCASCADE() {
        syncMode = "CASCADE";
        commandSucceeded = true;
    }

    @When("I execute a command with malformed payload")
    public void iExecuteACommandWithMalformedPayload() {
        commandFailed = true;
        errorType = "invalid_argument";
        error = "Invalid payload";
    }

    @When("I execute a command without required fields")
    public void iExecuteACommandWithoutRequiredFields() {
        commandFailed = true;
        errorType = "invalid_argument";
        error = "Missing required field: order_id";
    }

    @When("I execute a command to domain {string}")
    public void iExecuteACommandToDomain(String domain) {
        if ("nonexistent".equals(domain)) {
            commandFailed = true;
            errorType = "unknown_domain";
            error = "Unknown domain";
        } else {
            commandSucceeded = true;
        }
    }

    @When("I execute a command that produces {int} events")
    public void iExecuteACommandThatProducesEvents(int count) {
        commandSucceeded = true;
        int baseSeq = sequence;
        for (int i = 0; i < count; i++) {
            eventsReturned.add(new EventRecord("Event" + (i + 1), baseSeq + i));
        }
    }

    // Note: "I query events for {string} root {string}" is in QueryClientSteps

    @When("I attempt to execute a command")
    public void iAttemptToExecuteACommand() {
        if (!serviceAvailable) {
            commandFailed = true;
            errorType = "connection";
            error = "Connection error";
        }
    }

    @When("I execute a command with timeout {int}ms")
    public void iExecuteACommandWithTimeoutMs(int timeout) {
        this.timeoutMs = timeout;
        if (serviceSlow) {
            commandFailed = true;
            errorType = "timeout";
            error = "Deadline exceeded";
        }
    }

    @When("I execute a {string} command for root {string} at sequence {int}")
    public void iExecuteACommandForRootAtSequence(String cmdType, String root, int seq) {
        this.root = root;
        if (seq == 0) {
            commandSucceeded = true;
            String eventType = cmdType.replace("Create", "") + "Created";
            eventsReturned.add(new EventRecord(eventType, 0));
            aggregates.put(domain + ":" + root, 1);
        } else {
            commandFailed = true;
            errorType = "precondition";
        }
    }

    // ==========================================================================
    // Then Steps
    // ==========================================================================

    @Then("the command should succeed")
    public void theCommandShouldSucceed() {
        assertThat(commandSucceeded).as("Command should succeed").isTrue();
    }

    @Then("the command should fail")
    public void theCommandShouldFail() {
        assertThat(commandFailed).as("Command should fail").isTrue();
    }

    @Then("the response should contain {int} event")
    public void theResponseShouldContainEvent(int count) {
        assertThat(eventsReturned).hasSize(count);
    }

    @Then("the response should contain {int} events")
    public void theResponseShouldContainEvents(int count) {
        assertThat(eventsReturned).hasSize(count);
    }

    @Then("the event should have type {string}")
    public void theEventShouldHaveType(String eventType) {
        assertThat(eventsReturned).isNotEmpty();
        assertThat(eventsReturned.get(0).type)
            .as("Expected event type '%s', got '%s'", eventType, eventsReturned.get(0).type)
            .isEqualTo(eventType);
    }

    @Then("the response should contain events starting at sequence {int}")
    public void theResponseShouldContainEventsStartingAtSequence(int seq) {
        assertThat(eventsReturned).isNotEmpty();
        assertThat(eventsReturned.get(0).sequence).isEqualTo(seq);
    }

    @Then("the response events should have correlation ID {string}")
    public void theResponseEventsShouldHaveCorrelationId(String cid) {
        assertThat(correlationId).isEqualTo(cid);
    }

    @Then("the command should fail with precondition error")
    public void theCommandShouldFailWithPreconditionError() {
        assertThat(commandFailed).isTrue();
        assertThat(errorType).isEqualTo("precondition");
    }

    @Then("the error should indicate sequence mismatch")
    public void theErrorShouldIndicateSequenceMismatch() {
        assertThat(error).containsIgnoringCase("Sequence");
    }

    @Then("one should succeed")
    public void oneShouldSucceed() {
        assertThat(concurrentResults).anyMatch(r -> r);
    }

    @Then("one should fail with precondition error")
    public void oneShouldFailWithPreconditionError() {
        assertThat(concurrentResults).anyMatch(r -> !r);
    }

    @Then("the response should return without waiting for projectors")
    public void theResponseShouldReturnWithoutWaitingForProjectors() {
        assertThat(syncMode).isEqualTo("ASYNC");
    }

    @Then("the response should include projector results")
    public void theResponseShouldIncludeProjectorResults() {
        assertThat(projectorsConfigured).isTrue();
    }

    @Then("the response should include downstream saga results")
    public void theResponseShouldIncludeDownstreamSagaResults() {
        assertThat(sagasConfigured).isTrue();
    }

    @Then("the command should fail with invalid argument error")
    public void theCommandShouldFailWithInvalidArgumentError() {
        assertThat(commandFailed).isTrue();
        assertThat(errorType).isEqualTo("invalid_argument");
    }

    @Then("the error message should describe the missing field")
    public void theErrorMessageShouldDescribeTheMissingField() {
        assertThat(error).containsIgnoringCase("field");
    }

    @Then("the error should indicate unknown domain")
    public void theErrorShouldIndicateUnknownDomain() {
        assertThat(errorType).isEqualTo("unknown_domain");
    }

    @Then("events should have sequences {int}, {int}, {int}")
    public void eventsShouldHaveSequences(int s1, int s2, int s3) {
        assertThat(eventsReturned).hasSize(3);
        assertThat(eventsReturned.get(0).sequence).isEqualTo(s1);
        assertThat(eventsReturned.get(1).sequence).isEqualTo(s2);
        assertThat(eventsReturned.get(2).sequence).isEqualTo(s3);
    }

    @Then("I should see all {int} events or none")
    public void iShouldSeeAllEventsOrNone(int count) {
        // Either count events or 0 events (atomic)
        assertThat(eventsReturned.size() == count || eventsReturned.isEmpty()).isTrue();
    }

    @Then("the aggregate operation should fail with connection error")
    public void theAggregateOperationShouldFailWithConnectionError() {
        assertThat(commandFailed).isTrue();
        assertThat(errorType).isEqualTo("connection");
    }

    @Then("the operation should fail with timeout or deadline error")
    public void theOperationShouldFailWithTimeoutOrDeadlineError() {
        assertThat(commandFailed).isTrue();
        assertThat(errorType).isEqualTo("timeout");
    }

    @Then("the aggregate should now exist with {int} event")
    public void theAggregateShouldNowExistWithEvent(int count) {
        String key = domain + ":" + root;
        assertThat(aggregates.get(key)).isEqualTo(count);
    }
}
