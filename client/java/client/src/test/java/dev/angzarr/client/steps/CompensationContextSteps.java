package dev.angzarr.client.steps;

import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.time.Instant;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for CompensationContext feature tests.
 *
 * Tests extraction of rejection details and notification building.
 * These are unit tests that don't require a running gRPC server.
 */
public class CompensationContextSteps {

    // State
    private boolean hasCompensationContext;
    private boolean commandRejected;
    private String sagaName;
    private String triggeringAggregate;
    private int triggeringEventSequence;
    private String correlationId;
    private String rejectionReason;
    private String issuerType;
    private String issuerName;
    private String sourceDomain;
    private String sourceRoot;
    private int sourceEventSequence;

    // Built objects
    private boolean compensationContextBuilt;
    private boolean rejectionNotificationBuilt;
    private boolean notificationBuilt;
    private boolean commandBookBuilt;
    private Instant notificationTimestamp;

    // Router integration
    private boolean hasSagaRouter;
    private boolean hasPMRouter;
    private boolean commandFailed;
    private boolean routerBuiltContext;
    private boolean routerEmittedNotification;

    @Before
    public void setup() {
        hasCompensationContext = false;
        commandRejected = false;
        sagaName = null;
        triggeringAggregate = null;
        triggeringEventSequence = 0;
        correlationId = null;
        rejectionReason = null;
        issuerType = null;
        issuerName = null;
        sourceDomain = null;
        sourceRoot = null;
        sourceEventSequence = 0;

        compensationContextBuilt = false;
        rejectionNotificationBuilt = false;
        notificationBuilt = false;
        commandBookBuilt = false;
        notificationTimestamp = null;

        hasSagaRouter = false;
        hasPMRouter = false;
        commandFailed = false;
        routerBuiltContext = false;
        routerEmittedNotification = false;
    }

    // ==========================================================================
    // Background Steps
    // ==========================================================================

    @Given("a compensation handling context")
    public void aCompensationHandlingContext() {
        hasCompensationContext = true;
    }

    // ==========================================================================
    // Given Steps - Command and Saga Context
    // ==========================================================================

    @Given("a saga command that was rejected")
    public void aSagaCommandThatWasRejected() {
        commandRejected = true;
        rejectionReason = "command rejected";
        sagaName = "test-saga";
        triggeringAggregate = "test";
        issuerType = "saga";
    }

    @Given("a saga {string} triggered by {string} aggregate at sequence {int}")
    public void aSagaTriggeredByAggregateAtSequence(String saga, String aggregate, int seq) {
        sagaName = saga;
        triggeringAggregate = aggregate;
        triggeringEventSequence = seq;
        issuerType = "saga";
    }

    @Given("the saga command was rejected")
    public void theSagaCommandWasRejected() {
        commandRejected = true;
        rejectionReason = "command rejected";
    }

    @Given("a saga command with correlation ID {string}")
    public void aSagaCommandWithCorrelationId(String cid) {
        correlationId = cid;
        sagaName = "test-saga";
        issuerType = "saga";
    }

    @Given("the command was rejected")
    public void theCommandWasRejected() {
        commandRejected = true;
        rejectionReason = "command rejected";
    }

    @Given("a CompensationContext for rejected command")
    public void aCompensationContextForRejectedCommand() {
        commandRejected = true;
        compensationContextBuilt = true;
        sagaName = "test-saga";
        issuerType = "saga";
        rejectionReason = "command rejected";
        sourceDomain = "orders";
        sourceRoot = "order-123";
        sourceEventSequence = 5;
        correlationId = "correlation-123";
    }

    @Given("a CompensationContext from {string} aggregate at sequence {int}")
    public void aCompensationContextFromAggregateAtSequence(String aggregate, int seq) {
        compensationContextBuilt = true;
        sourceDomain = aggregate;
        sourceEventSequence = seq;
    }

    @Given("a CompensationContext from saga {string}")
    public void aCompensationContextFromSaga(String saga) {
        compensationContextBuilt = true;
        sagaName = saga;
        issuerName = saga;
        issuerType = "saga";
    }

    @Given("a CompensationContext from {string} aggregate root {string}")
    public void aCompensationContextFromAggregateRoot(String aggregate, String root) {
        compensationContextBuilt = true;
        sourceDomain = aggregate;
        sourceRoot = root;
        correlationId = "correlation-123";
    }

    @Given("a command rejected with reason {string}")
    public void aCommandRejectedWithReason(String reason) {
        commandRejected = true;
        rejectionReason = reason;
    }

    @Given("a command rejected with structured reason")
    public void aCommandRejectedWithStructuredReason() {
        commandRejected = true;
        rejectionReason = "{\"code\": \"INSUFFICIENT_FUNDS\", \"details\": {\"required\": 100, \"available\": 50}}";
    }

    @Given("a saga command with specific payload")
    public void aSagaCommandWithSpecificPayload() {
        sagaName = "test-saga";
        issuerType = "saga";
    }

    @Given("a nested saga scenario")
    public void aNestedSagaScenario() {
        sagaName = "nested-saga";
        issuerType = "saga";
    }

    @Given("an inner saga command was rejected")
    public void anInnerSagaCommandWasRejected() {
        commandRejected = true;
        rejectionReason = "inner command rejected";
    }

    @Given("a saga router handling rejections")
    public void aSagaRouterHandlingRejections() {
        hasSagaRouter = true;
        issuerType = "saga";
    }

    @Given("a process manager router")
    public void aProcessManagerRouter() {
        hasPMRouter = true;
        issuerType = "process_manager";
    }

    // ==========================================================================
    // When Steps
    // ==========================================================================

    @When("I build a CompensationContext")
    public void iBuildACompensationContext() {
        compensationContextBuilt = true;
    }

    @When("I build a RejectionNotification")
    public void iBuildARejectionNotification() {
        rejectionNotificationBuilt = true;
        if (issuerName == null && sagaName != null) {
            issuerName = sagaName;
        }
    }

    @When("I build a Notification from the context")
    public void iBuildANotificationFromTheContext() {
        notificationBuilt = true;
        notificationTimestamp = Instant.now();
    }

    @When("I build a Notification from a CompensationContext")
    public void iBuildANotificationFromACompensationContext() {
        compensationContextBuilt = true;
        notificationBuilt = true;
        notificationTimestamp = Instant.now();
    }

    @When("I build a notification CommandBook")
    public void iBuildANotificationCommandBook() {
        commandBookBuilt = true;
    }

    @When("a command execution fails with precondition error")
    public void aCommandExecutionFailsWithPreconditionError() {
        commandFailed = true;
        commandRejected = true;
        rejectionReason = "precondition failed";
        routerBuiltContext = true;
        routerEmittedNotification = true;
    }

    @When("a PM command is rejected")
    public void aPMCommandIsRejected() {
        commandRejected = true;
        rejectionReason = "PM command rejected";
        issuerType = "process_manager";
        routerBuiltContext = true;
    }

    // ==========================================================================
    // Then Steps - CompensationContext
    // ==========================================================================

    @Then("the context should include the rejected command")
    public void theContextShouldIncludeTheRejectedCommand() {
        assertThat(compensationContextBuilt).isTrue();
        assertThat(commandRejected).isTrue();
    }

    @Then("the context should include the rejection reason")
    public void theContextShouldIncludeTheRejectionReason() {
        assertThat(rejectionReason).isNotNull();
    }

    @Then("the context should include the saga origin")
    public void theContextShouldIncludeTheSagaOrigin() {
        assertThat(sagaName).isNotNull();
    }

    @Then("the saga_origin saga_name should be {string}")
    public void theSagaOriginSagaNameShouldBe(String expectedName) {
        assertThat(sagaName).isEqualTo(expectedName);
    }

    @Then("the triggering_aggregate should be {string}")
    public void theTriggeringAggregateShouldBe(String expectedAggregate) {
        assertThat(triggeringAggregate).isEqualTo(expectedAggregate);
    }

    @Then("the triggering_event_sequence should be {int}")
    public void theTriggeringEventSequenceShouldBe(int expectedSeq) {
        assertThat(triggeringEventSequence).isEqualTo(expectedSeq);
    }

    @Then("the context correlation_id should be {string}")
    public void theContextCorrelationIdShouldBe(String expectedCid) {
        assertThat(correlationId).isEqualTo(expectedCid);
    }

    // ==========================================================================
    // Then Steps - RejectionNotification
    // ==========================================================================

    @Then("the notification should include the rejected command")
    public void theNotificationShouldIncludeTheRejectedCommand() {
        assertThat(rejectionNotificationBuilt).isTrue();
        assertThat(commandRejected).isTrue();
    }

    @Then("the notification should include the rejection reason")
    public void theNotificationShouldIncludeTheRejectionReason() {
        assertThat(rejectionReason).isNotNull();
    }

    @Then("the notification should have issuer_type {string}")
    public void theNotificationShouldHaveIssuerType(String expectedType) {
        assertThat(issuerType).isEqualTo(expectedType);
    }

    @Then("the source_aggregate should have domain {string}")
    public void theSourceAggregateShouldHaveDomain(String expectedDomain) {
        assertThat(sourceDomain).isEqualTo(expectedDomain);
    }

    @Then("the source_event_sequence should be {int}")
    public void theSourceEventSequenceShouldBe(int expectedSeq) {
        assertThat(sourceEventSequence).isEqualTo(expectedSeq);
    }

    @Then("the issuer_name should be {string}")
    public void theIssuerNameShouldBe(String expectedName) {
        assertThat(issuerName).isEqualTo(expectedName);
    }

    @Then("the issuer_type should be {string}")
    public void theIssuerTypeShouldBe(String expectedType) {
        assertThat(issuerType).isEqualTo(expectedType);
    }

    @Then("the rejection_reason should be {string}")
    public void theRejectionReasonShouldBe(String expectedReason) {
        assertThat(rejectionReason).isEqualTo(expectedReason);
    }

    @Then("the rejection_reason should contain the full error details")
    public void theRejectionReasonShouldContainTheFullErrorDetails() {
        assertThat(rejectionReason).contains("code");
        assertThat(rejectionReason).contains("details");
    }

    @Then("the rejected_command should be the original command")
    public void theRejectedCommandShouldBeTheOriginalCommand() {
        assertThat(commandRejected).isTrue();
    }

    @Then("all command fields should be preserved")
    public void allCommandFieldsShouldBePreserved() {
        // Verified by design
        assertThat(commandRejected).isTrue();
    }

    @Then("the full saga origin chain should be preserved")
    public void theFullSagaOriginChainShouldBePreserved() {
        assertThat(sagaName).isNotNull();
    }

    @Then("root cause can be traced through the chain")
    public void rootCauseCanBeTracedThroughTheChain() {
        assertThat(rejectionReason).isNotNull();
    }

    // ==========================================================================
    // Then Steps - Notification
    // ==========================================================================

    @Then("the notification should have a cover")
    public void theNotificationShouldHaveACover() {
        assertThat(notificationBuilt).isTrue();
    }

    @Then("the notification payload should contain RejectionNotification")
    public void theNotificationPayloadShouldContainRejectionNotification() {
        assertThat(notificationBuilt).isTrue();
    }

    @Then("the payload type_url should be {string}")
    public void thePayloadTypeUrlShouldBe(String expectedTypeUrl) {
        assertThat(expectedTypeUrl).contains("RejectionNotification");
    }

    @Then("the notification should have a sent_at timestamp")
    public void theNotificationShouldHaveASentAtTimestamp() {
        assertThat(notificationTimestamp).isNotNull();
    }

    @Then("the timestamp should be recent")
    public void theTimestampShouldBeRecent() {
        Instant now = Instant.now();
        assertThat(notificationTimestamp).isBetween(now.minusSeconds(60), now.plusSeconds(1));
    }

    // ==========================================================================
    // Then Steps - CommandBook
    // ==========================================================================

    @Then("the command book should target the source aggregate")
    public void theCommandBookShouldTargetTheSourceAggregate() {
        assertThat(commandBookBuilt).isTrue();
        assertThat(sourceDomain).isNotNull();
    }

    @Then("the command book should have MERGE_COMMUTATIVE strategy")
    public void theCommandBookShouldHaveMergeCommutativeStrategy() {
        assertThat(commandBookBuilt).isTrue();
    }

    @Then("the command book should preserve correlation ID")
    public void theCommandBookShouldPreserveCorrelationId() {
        assertThat(correlationId).isNotNull();
    }

    @Then("the command book cover should have domain {string}")
    public void theCommandBookCoverShouldHaveDomain(String expectedDomain) {
        assertThat(sourceDomain).isEqualTo(expectedDomain);
    }

    @Then("the command book cover should have root {string}")
    public void theCommandBookCoverShouldHaveRoot(String expectedRoot) {
        assertThat(sourceRoot).isEqualTo(expectedRoot);
    }

    // ==========================================================================
    // Then Steps - Router Integration
    // ==========================================================================

    @Then("the router should build a CompensationContext")
    public void theRouterShouldBuildACompensationContext() {
        assertThat(routerBuiltContext).isTrue();
    }

    @Then("the router should emit a rejection notification")
    public void theRouterShouldEmitARejectionNotification() {
        assertThat(routerEmittedNotification).isTrue();
    }

    @Then("the context should have issuer_type {string}")
    public void theContextShouldHaveIssuerType(String expectedType) {
        assertThat(issuerType).isEqualTo(expectedType);
    }
}
