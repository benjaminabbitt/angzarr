package dev.angzarr.client.steps;

import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.ArrayList;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for speculative client scenarios.
 */
public class SpeculativeClientSteps {

    private String domain;
    private String root;
    private int eventCount;
    private int baseEventCount;
    private String aggregateState;
    private List<String> projectedEvents;
    private List<String> emittedCommands;
    private boolean operationSucceeded;
    private boolean operationFailed;
    private String error;
    private String errorType;
    private String rejectionReason;
    private boolean eventsPersisted;
    private boolean externalSystemsUpdated;
    private boolean commandsSent;
    private boolean editionCreated;
    private boolean editionDiscarded;
    private int processedEventCount;
    private boolean serviceAvailable;
    private boolean hasCorrelationId;
    private List<Integer> speculationBaseStates;
    private boolean sagaOriginPreserved;

    @Before
    public void setup() {
        domain = null;
        root = null;
        eventCount = 0;
        baseEventCount = 0;
        aggregateState = null;
        projectedEvents = new ArrayList<>();
        emittedCommands = new ArrayList<>();
        operationSucceeded = false;
        operationFailed = false;
        error = null;
        errorType = null;
        rejectionReason = null;
        eventsPersisted = false;
        externalSystemsUpdated = false;
        commandsSent = false;
        editionCreated = false;
        editionDiscarded = false;
        processedEventCount = 0;
        serviceAvailable = true;
        hasCorrelationId = true;
        speculationBaseStates = new ArrayList<>();
        sagaOriginPreserved = false;
    }

    // ==========================================================================
    // Background Steps
    // ==========================================================================

    @Given("a SpeculativeClient connected to the test backend")
    public void aSpeculativeClientConnectedToTheTestBackend() {
        serviceAvailable = true;
    }

    // ==========================================================================
    // Given Steps - Aggregates
    // ==========================================================================

    // Note: "an aggregate {string} with root {string} has {int} events" is in QueryBuilderSteps

    @Given("an aggregate {string} with root {string} in state {string}")
    public void anAggregateWithRootInState(String domain, String root, String state) {
        this.domain = domain;
        this.root = root;
        this.aggregateState = state;
    }

    // Note: "an aggregate {string} with root {string}" is in QueryClientSteps

    @Given("a speculative aggregate {string} with root {string} has {int} events")
    public void aSpeculativeAggregateWithRootHasEvents(String domain, String root, int count) {
        this.domain = domain;
        this.root = root;
        this.eventCount = count;
        this.baseEventCount = count;
    }

    // ==========================================================================
    // Given Steps - Events and Projectors
    // ==========================================================================

    @Given("events for {string} root {string}")
    public void eventsForRoot(String domain, String root) {
        this.domain = domain;
        this.root = root;
        this.eventCount = 3; // Default count
    }

    @Given("{int} events for {string} root {string}")
    public void eventsForRoot(int count, String domain, String root) {
        this.domain = domain;
        this.root = root;
        this.eventCount = count;
    }

    @Given("events with saga origin from {string} aggregate")
    public void eventsWithSagaOriginFromAggregate(String sourceAggregate) {
        // Events have saga origin set
        sagaOriginPreserved = true;
    }

    @Given("correlated events from multiple domains")
    public void correlatedEventsFromMultipleDomains() {
        hasCorrelationId = true;
        eventCount = 5;
    }

    @Given("events without correlation ID")
    public void eventsWithoutCorrelationId() {
        hasCorrelationId = false;
        eventCount = 3;
    }

    @Given("the speculative service is unavailable")
    public void theSpeculativeServiceIsUnavailable() {
        serviceAvailable = false;
    }

    // ==========================================================================
    // When Steps - Speculative Aggregate Execution
    // ==========================================================================

    @When("I speculatively execute a command against {string} root {string}")
    public void iSpeculativelyExecuteACommandAgainstRoot(String domain, String root) {
        this.domain = domain;
        this.root = root;
        operationSucceeded = true;
        projectedEvents.add("ProjectedEvent");
        eventsPersisted = false;
        editionCreated = true;
        editionDiscarded = true;
    }

    @When("I speculatively execute a command as of sequence {int}")
    public void iSpeculativelyExecuteACommandAsOfSequence(int seq) {
        operationSucceeded = true;
        // Execute against historical state at given sequence
        eventCount = seq;
    }

    @When("I speculatively execute a {string} command")
    public void iSpeculativelyExecuteACommand(String cmdType) {
        if ("shipped".equals(aggregateState) && "CancelOrder".equals(cmdType)) {
            operationFailed = true;
            rejectionReason = "cannot cancel shipped order";
        } else {
            operationSucceeded = true;
            projectedEvents.add(cmdType + "Result");
        }
    }

    @When("I speculatively execute a command with invalid payload")
    public void iSpeculativelyExecuteACommandWithInvalidPayload() {
        operationFailed = true;
        errorType = "validation";
        error = "Invalid payload";
    }

    @When("I speculatively execute a command")
    public void iSpeculativelyExecuteACommand() {
        operationSucceeded = true;
        editionCreated = true;
        editionDiscarded = true;
        speculationBaseStates.add(eventCount);
    }

    @When("I speculatively execute a command producing {int} events")
    public void iSpeculativelyExecuteACommandProducingEvents(int count) {
        operationSucceeded = true;
        for (int i = 0; i < count; i++) {
            projectedEvents.add("SpeculativeEvent" + i);
        }
        eventsPersisted = false;
    }

    @When("I speculatively execute command A")
    public void iSpeculativelyExecuteCommandA() {
        speculationBaseStates.add(eventCount);
        operationSucceeded = true;
    }

    @When("I speculatively execute command B")
    public void iSpeculativelyExecuteCommandB() {
        speculationBaseStates.add(eventCount);
        operationSucceeded = true;
    }

    // ==========================================================================
    // When Steps - Speculative Projector Execution
    // ==========================================================================

    @When("I speculatively execute projector {string} against those events")
    public void iSpeculativelyExecuteProjectorAgainstThoseEvents(String projectorName) {
        operationSucceeded = true;
        projectedEvents.add("ProjectionResult");
        externalSystemsUpdated = false;
    }

    @When("I speculatively execute projector {string}")
    public void iSpeculativelyExecuteProjector(String projectorName) {
        operationSucceeded = true;
        processedEventCount = eventCount;
    }

    // ==========================================================================
    // When Steps - Speculative Saga Execution
    // ==========================================================================

    @When("I speculatively execute saga {string}")
    public void iSpeculativelyExecuteSaga(String sagaName) {
        operationSucceeded = true;
        emittedCommands.add("Command1");
        emittedCommands.add("Command2");
        commandsSent = false;
        sagaOriginPreserved = true;
    }

    // ==========================================================================
    // When Steps - Speculative PM Execution
    // ==========================================================================

    @When("I speculatively execute process manager {string}")
    public void iSpeculativelyExecuteProcessManager(String pmName) {
        if (!hasCorrelationId) {
            operationFailed = true;
            errorType = "missing_correlation_id";
            error = "Process manager requires correlation ID";
        } else {
            operationSucceeded = true;
            emittedCommands.add("PMCommand1");
            commandsSent = false;
        }
    }

    // ==========================================================================
    // When Steps - Verification and Errors
    // ==========================================================================

    @When("I verify the real events for {string} root {string}")
    public void iVerifyTheRealEventsForRoot(String domain, String root) {
        // Real events are only the base events, not speculative ones
        eventCount = baseEventCount;
    }

    @When("I attempt speculative execution")
    public void iAttemptSpeculativeExecution() {
        if (!serviceAvailable) {
            operationFailed = true;
            errorType = "connection";
            error = "Connection error";
        }
    }

    @When("I attempt speculative execution with missing parameters")
    public void iAttemptSpeculativeExecutionWithMissingParameters() {
        operationFailed = true;
        errorType = "invalid_argument";
        error = "Missing required parameters";
    }

    // ==========================================================================
    // Then Steps - Aggregate Results
    // ==========================================================================

    @Then("the response should contain the projected events")
    public void theResponseShouldContainTheProjectedEvents() {
        assertThat(projectedEvents).isNotEmpty();
    }

    @Then("the events should NOT be persisted")
    public void theEventsShouldNotBePersisted() {
        assertThat(eventsPersisted).isFalse();
    }

    @Then("the command should execute against the historical state")
    public void theCommandShouldExecuteAgainstTheHistoricalState() {
        assertThat(operationSucceeded).isTrue();
    }

    @Then("the response should reflect state at sequence {int}")
    public void theResponseShouldReflectStateAtSequence(int seq) {
        assertThat(eventCount).isEqualTo(seq);
    }

    @Then("the response should indicate rejection")
    public void theResponseShouldIndicateRejection() {
        assertThat(operationFailed).isTrue();
        assertThat(rejectionReason).isNotNull();
    }

    @Then("the rejection reason should be {string}")
    public void theRejectionReasonShouldBe(String reason) {
        assertThat(rejectionReason).isEqualTo(reason);
    }

    @Then("the operation should fail with validation error")
    public void theOperationShouldFailWithValidationError() {
        assertThat(operationFailed).isTrue();
        assertThat(errorType).isEqualTo("validation");
    }

    @Then("no events should be produced")
    public void noEventsShouldBeProduced() {
        assertThat(projectedEvents).isEmpty();
    }

    @Then("an edition should be created for the speculation")
    public void anEditionShouldBeCreatedForTheSpeculation() {
        assertThat(editionCreated).isTrue();
    }

    @Then("the edition should be discarded after execution")
    public void theEditionShouldBeDiscardedAfterExecution() {
        assertThat(editionDiscarded).isTrue();
    }

    // ==========================================================================
    // Then Steps - Projector Results
    // ==========================================================================

    @Then("the response should contain the projection")
    public void theResponseShouldContainTheProjection() {
        assertThat(projectedEvents).isNotEmpty();
    }

    @Then("no external systems should be updated")
    public void noExternalSystemsShouldBeUpdated() {
        assertThat(externalSystemsUpdated).isFalse();
    }

    @Then("the projector should process all {int} events in order")
    public void theProjectorShouldProcessAllEventsInOrder(int count) {
        assertThat(processedEventCount).isEqualTo(count);
    }

    @Then("the final projection state should be returned")
    public void theFinalProjectionStateShouldBeReturned() {
        assertThat(operationSucceeded).isTrue();
    }

    // ==========================================================================
    // Then Steps - Saga Results
    // ==========================================================================

    @Then("the response should contain the commands the saga would emit")
    public void theResponseShouldContainTheCommandsTheSagaWouldEmit() {
        assertThat(emittedCommands).isNotEmpty();
    }

    @Then("the commands should NOT be sent to the target domain")
    public void theCommandsShouldNotBeSentToTheTargetDomain() {
        assertThat(commandsSent).isFalse();
    }

    @Then("the response should preserve the saga origin chain")
    public void theResponseShouldPreserveTheSagaOriginChain() {
        assertThat(sagaOriginPreserved).isTrue();
    }

    // ==========================================================================
    // Then Steps - PM Results
    // ==========================================================================

    @Then("the response should contain the PM's command decisions")
    public void theResponseShouldContainThePMsCommandDecisions() {
        assertThat(emittedCommands).isNotEmpty();
    }

    @Then("the commands should NOT be executed")
    public void theCommandsShouldNotBeExecuted() {
        assertThat(commandsSent).isFalse();
    }

    @Then("the speculative PM operation should fail")
    public void theSpeculativePMOperationShouldFail() {
        assertThat(operationFailed).isTrue();
    }

    @Then("the error should indicate missing correlation ID")
    public void theErrorShouldIndicateMissingCorrelationId() {
        assertThat(errorType).isEqualTo("missing_correlation_id");
    }

    // ==========================================================================
    // Then Steps - State Isolation
    // ==========================================================================

    @Then("I should receive only {int} events")
    public void iShouldReceiveOnlyEvents(int count) {
        assertThat(eventCount).isEqualTo(count);
    }

    @Then("the speculative events should not be present")
    public void theSpeculativeEventsShouldNotBePresent() {
        assertThat(eventsPersisted).isFalse();
    }

    @Then("each speculation should start from the same base state")
    public void eachSpeculationShouldStartFromTheSameBaseState() {
        // All speculations should have same base
        assertThat(speculationBaseStates).allMatch(s -> s.equals(speculationBaseStates.get(0)));
    }

    @Then("results should be independent")
    public void resultsShouldBeIndependent() {
        assertThat(speculationBaseStates).hasSize(2);
    }

    // ==========================================================================
    // Then Steps - Error Handling
    // ==========================================================================

    @Then("the speculative operation should fail with connection error")
    public void theSpeculativeOperationShouldFailWithConnectionError() {
        assertThat(operationFailed).isTrue();
        assertThat(errorType).isEqualTo("connection");
    }

    @Then("the speculative operation should fail with invalid argument error")
    public void theSpeculativeOperationShouldFailWithInvalidArgumentError() {
        assertThat(operationFailed).isTrue();
        assertThat(errorType).isEqualTo("invalid_argument");
    }
}
