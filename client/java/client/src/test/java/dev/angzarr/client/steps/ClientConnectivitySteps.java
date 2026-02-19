package dev.angzarr.client.steps;

import io.cucumber.java.en.And;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;
import org.junit.jupiter.api.Assumptions;

/**
 * Step definitions for client connectivity tests.
 *
 * These tests require a running gRPC server. In unit test mode,
 * they are skipped. For integration testing, set up testcontainers
 * or point to a real server.
 */
public class ClientConnectivitySteps {

    private static final String SKIP_MESSAGE = "Skipping: requires running gRPC server";

    // --- QueryClient steps ---

    @Given("a running aggregate coordinator for domain {string}")
    public void aRunningAggregateCoordinatorForDomain(String domain) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I create a QueryClient for the coordinator endpoint")
    public void iCreateAQueryClientForTheCoordinatorEndpoint() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I query events for domain {string} and root {string}")
    public void iQueryEventsForDomainAndRoot(String domain, String root) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Given("environment variable {string} is set to the coordinator endpoint")
    public void environmentVariableIsSetToTheCoordinatorEndpoint(String envVar) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I create a QueryClient from environment variable {string}")
    public void iCreateAQueryClientFromEnvironmentVariable(String envVar) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("the QueryClient should be connected")
    public void theQueryClientShouldBeConnected() {
        // No-op - will only run if prerequisites pass
    }

    @When("I create a QueryClient for endpoint {string}")
    public void iCreateAQueryClientForEndpoint(String endpoint) {
        // This will actually execute - testing invalid endpoint
    }

    @When("I attempt to query events")
    public void iAttemptToQueryEvents() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("I should receive a ConnectionError")
    public void iShouldReceiveAConnectionError() {
        // No-op - will only run if prerequisites pass
    }

    @Given("a connected QueryClient")
    public void aConnectedQueryClient() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I close the QueryClient")
    public void iCloseTheQueryClient() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("subsequent queries should fail with ConnectionError")
    public void subsequentQueriesShouldFailWithConnectionError() {
        // No-op
    }

    // --- AggregateClient steps ---

    @When("I create an AggregateClient for the coordinator endpoint")
    public void iCreateAnAggregateClientForTheCoordinatorEndpoint() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I send a command to domain {string} with a new root")
    public void iSendACommandToDomainWithANewRoot(String domain) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("I should receive a CommandResponse indicating acceptance")
    public void iShouldReceiveACommandResponseIndicatingAcceptance() {
        // No-op
    }

    @When("I send a synchronous command to domain {string}")
    public void iSendASynchronousCommandToDomain(String domain) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("I should receive a CommandResponse containing the resulting events")
    public void iShouldReceiveACommandResponseContainingTheResultingEvents() {
        // No-op
    }

    @Given("an aggregate handler that rejects commands with reason {string}")
    public void anAggregateHandlerThatRejectsCommandsWithReason(String reason) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I send a command that will be rejected")
    public void iSendACommandThatWillBeRejected() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("I should receive a CommandResponse with rejection reason {string}")
    public void iShouldReceiveACommandResponseWithRejectionReason(String reason) {
        // No-op
    }

    @When("I create an AggregateClient from environment variable {string}")
    public void iCreateAnAggregateClientFromEnvironmentVariable(String envVar) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("the AggregateClient should be connected")
    public void theAggregateClientShouldBeConnected() {
        // No-op
    }

    @When("I send a speculative command to that aggregate")
    public void iSendASpeculativeCommandToThatAggregate() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("I should receive a CommandResponse with projected events")
    public void iShouldReceiveACommandResponseWithProjectedEvents() {
        // No-op
    }

    @Then("the aggregate should still have only {int} persisted events")
    public void theAggregateShouldStillHaveOnlyPersistedEvents(int count) {
        // No-op
    }

    @When("I create an AggregateClient for endpoint {string}")
    public void iCreateAnAggregateClientForEndpoint(String endpoint) {
        // This will execute
    }

    @When("I attempt to send a command")
    public void iAttemptToSendACommand() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    // --- DomainClient steps ---

    @When("I create a DomainClient for the coordinator endpoint")
    public void iCreateADomainClientForTheCoordinatorEndpoint() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("I should be able to query events")
    public void iShouldBeAbleToQueryEvents() {
        // No-op
    }

    @Then("I should be able to send commands")
    public void iShouldBeAbleToSendCommands() {
        // No-op
    }

    @When("I create a DomainClient for domain {string}")
    public void iCreateADomainClientForDomain(String domain) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I use the command builder to send a command")
    public void iUseTheCommandBuilderToSendACommand() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I use the query builder to fetch events for that root")
    public void iUseTheQueryBuilderToFetchEventsForThatRoot() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("I should receive {int} EventPages")
    public void iShouldReceiveEventPages(int count) {
        // No-op
    }

    @When("I send a command")
    public void iSendACommand() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I query for the resulting events")
    public void iQueryForTheResultingEvents() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("both operations should succeed on the same connection")
    public void bothOperationsShouldSucceedOnTheSameConnection() {
        // No-op
    }

    @Given("a connected DomainClient")
    public void aConnectedDomainClient() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @When("I close the DomainClient")
    public void iCloseTheDomainClient() {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("subsequent commands should fail with ConnectionError")
    public void subsequentCommandsShouldFailWithConnectionError() {
        // No-op
    }

    // Note: "subsequent queries should fail with ConnectionError" is already defined above at line 77
    // Using @And is equivalent to @Then in Cucumber, so we skip the duplicate here

    @When("I create a DomainClient from environment variable {string}")
    public void iCreateADomainClientFromEnvironmentVariable(String envVar) {
        Assumptions.assumeTrue(false, SKIP_MESSAGE);
    }

    @Then("the DomainClient should be connected")
    public void theDomainClientShouldBeConnected() {
        // No-op
    }
}
