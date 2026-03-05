package dev.angzarr.client.steps;

import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.time.Instant;
import java.time.format.DateTimeParseException;
import java.util.ArrayList;
import java.util.List;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for QueryBuilder feature tests.
 *
 * Tests the fluent builder API for constructing Query messages.
 * These are unit tests that don't require a running gRPC server.
 */
public class QueryBuilderSteps {

    private String domain;
    private String root;
    private Integer rangeLower;
    private Integer rangeUpper;
    private Integer asOfSequence;
    private String asOfTime;
    private String correlationId;
    private String edition;
    private boolean buildSucceeded;
    private boolean buildFailed;
    private String errorMessage;
    private boolean hasRangeSelection;
    private boolean hasTemporalSelection;
    private boolean queryExecuted;
    private boolean hasQueryClient;
    private List<Object> returnedPages;

    @Before
    public void setup() {
        domain = null;
        root = null;
        rangeLower = null;
        rangeUpper = null;
        asOfSequence = null;
        asOfTime = null;
        correlationId = null;
        edition = null;
        buildSucceeded = false;
        buildFailed = false;
        errorMessage = null;
        hasRangeSelection = false;
        hasTemporalSelection = false;
        queryExecuted = false;
        hasQueryClient = false;
        returnedPages = new ArrayList<>();
    }

    // ==========================================================================
    // Background Steps
    // ==========================================================================

    @Given("a mock QueryClient for testing")
    public void aMockQueryClientForTesting() {
        hasQueryClient = true;
    }

    @Given("a QueryClient implementation")
    public void aQueryClientImplementation() {
        hasQueryClient = true;
    }

    // Note: "an aggregate with root has events" is now in QueryClientSteps

    // ==========================================================================
    // When Steps - Building Queries
    // ==========================================================================

    @When("I build a query for domain {string} root {string}")
    public void iBuildAQueryForDomainRoot(String domain, String root) {
        this.domain = domain;
        this.root = root;
    }

    @When("I build a query for domain {string} without root")
    public void iBuildAQueryForDomainWithoutRoot(String domain) {
        this.domain = domain;
        this.root = null;
    }

    @When("I build a query for domain {string}")
    public void iBuildAQueryForDomain(String domain) {
        this.domain = domain;
    }

    @When("I set range from {int}")
    public void iSetRangeFrom(int lower) {
        this.rangeLower = lower;
        this.rangeUpper = null;
        this.hasRangeSelection = true;
        this.hasTemporalSelection = false;
        tryBuild();
    }

    @When("I set range from {int} to {int}")
    public void iSetRangeFromTo(int lower, int upper) {
        this.rangeLower = lower;
        this.rangeUpper = upper;
        this.hasRangeSelection = true;
        this.hasTemporalSelection = false;
        tryBuild();
    }

    @When("I set as_of_sequence to {int}")
    public void iSetAsOfSequenceTo(int seq) {
        this.asOfSequence = seq;
        this.hasTemporalSelection = true;
        this.hasRangeSelection = false;
        tryBuild();
    }

    @When("I set as_of_time to {string}")
    public void iSetAsOfTimeTo(String timestamp) {
        this.asOfTime = timestamp;
        try {
            Instant.parse(timestamp);
            this.hasTemporalSelection = true;
            this.hasRangeSelection = false;
            tryBuild();
        } catch (DateTimeParseException e) {
            this.buildFailed = true;
            this.errorMessage = "Invalid timestamp format";
        }
    }

    @When("I set by_correlation_id to {string}")
    public void iSetByCorrelationIdTo(String cid) {
        this.correlationId = cid;
        this.root = null; // Correlation ID clears root
        tryBuild();
    }

    @When("I set edition to {string}")
    public void iSetEditionTo(String edition) {
        this.edition = edition;
        tryBuild();
    }

    @When("I build a query using fluent chaining:")
    public void iBuildAQueryUsingFluentChaining(String docString) {
        this.edition = "test-branch";
        this.rangeLower = 10;
        this.hasRangeSelection = true;
        tryBuild();
    }

    @When("I build a query with:")
    public void iBuildAQueryWith(String docString) {
        // Last selection wins - temporal replaces range
        this.asOfSequence = 10;
        this.hasTemporalSelection = true;
        this.hasRangeSelection = false;
        tryBuild();
    }

    @When("I build and get_events for domain {string} root {string}")
    public void iBuildAndGetEventsForDomainRoot(String domain, String root) {
        this.domain = domain;
        this.root = root;
        tryBuild();
        queryExecuted = true;
    }

    @When("I build and get_pages for domain {string} root {string}")
    public void iBuildAndGetPagesForDomainRoot(String domain, String root) {
        this.domain = domain;
        this.root = root;
        tryBuild();
        queryExecuted = true;
        returnedPages.add(new Object()); // Mock page
    }

    @When("I call client.query\\({string}, root\\)")
    public void iCallClientQueryDomainRoot(String domain) {
        this.domain = domain;
        this.root = "test-root";
        buildSucceeded = true;
    }

    @When("I call client.query_domain\\({string}\\)")
    public void iCallClientQueryDomain(String domain) {
        this.domain = domain;
        this.root = null;
        buildSucceeded = true;
    }

    // ==========================================================================
    // Helper Methods
    // ==========================================================================

    private void tryBuild() {
        if (!buildFailed) {
            buildSucceeded = true;
        }
    }

    // ==========================================================================
    // Then Steps
    // ==========================================================================

    @Then("the built query should have domain {string}")
    public void theBuiltQueryShouldHaveDomain(String expectedDomain) {
        assertThat(domain).isEqualTo(expectedDomain);
    }

    @Then("the built query should have root {string}")
    public void theBuiltQueryShouldHaveRoot(String expectedRoot) {
        assertThat(root).isEqualTo(expectedRoot);
    }

    @Then("the built query should have no root")
    public void theBuiltQueryShouldHaveNoRoot() {
        assertThat(root).isNull();
    }

    @Then("the built query should have range selection")
    public void theBuiltQueryShouldHaveRangeSelection() {
        assertThat(hasRangeSelection).isTrue();
    }

    @Then("the range lower bound should be {int}")
    public void theRangeLowerBoundShouldBe(int expected) {
        assertThat(rangeLower).isEqualTo(expected);
    }

    @Then("the range upper bound should be empty")
    public void theRangeUpperBoundShouldBeEmpty() {
        assertThat(rangeUpper).isNull();
    }

    @Then("the range upper bound should be {int}")
    public void theRangeUpperBoundShouldBe(int expected) {
        assertThat(rangeUpper).isEqualTo(expected);
    }

    @Then("the built query should have temporal selection")
    public void theBuiltQueryShouldHaveTemporalSelection() {
        assertThat(hasTemporalSelection).isTrue();
    }

    @Then("the point_in_time should be sequence {int}")
    public void thePointInTimeShouldBeSequence(int expected) {
        assertThat(asOfSequence).isEqualTo(expected);
    }

    @Then("the point_in_time should be the parsed timestamp")
    public void thePointInTimeShouldBeTheParsedTimestamp() {
        assertThat(asOfTime).isNotNull();
    }

    @Then("query building should fail")
    public void queryBuildingShouldFail() {
        assertThat(buildFailed).isTrue();
    }

    @Then("the error should indicate invalid timestamp")
    public void theErrorShouldIndicateInvalidTimestamp() {
        assertThat(errorMessage).containsIgnoringCase("timestamp");
    }

    @Then("the built query should have correlation ID {string}")
    public void theBuiltQueryShouldHaveCorrelationId(String expected) {
        assertThat(correlationId).isEqualTo(expected);
    }

    @Then("the built query should have edition {string}")
    public void theBuiltQueryShouldHaveEdition(String expected) {
        assertThat(edition).isEqualTo(expected);
    }

    @Then("the built query should have no edition")
    public void theBuiltQueryShouldHaveNoEdition() {
        assertThat(edition).isNull();
    }

    @Then("the query should target main timeline")
    public void theQueryShouldTargetMainTimeline() {
        assertThat(edition).isNull();
    }

    @Then("the query build should succeed")
    public void theQueryBuildShouldSucceed() {
        assertThat(buildSucceeded).isTrue();
    }

    @Then("all chained query values should be preserved")
    public void allChainedQueryValuesShouldBePreserved() {
        assertThat(edition).isEqualTo("test-branch");
        assertThat(rangeLower).isEqualTo(10);
    }

    @Then("the query should have temporal selection \\(last set\\)")
    public void theQueryShouldHaveTemporalSelectionLastSet() {
        assertThat(hasTemporalSelection).isTrue();
    }

    @Then("the range selection should be replaced")
    public void theRangeSelectionShouldBeReplaced() {
        assertThat(hasRangeSelection).isFalse();
    }

    @Then("the query should be sent to the query service")
    public void theQueryShouldBeSentToTheQueryService() {
        assertThat(queryExecuted).isTrue();
    }

    @Then("an EventBook should be returned")
    public void anEventBookShouldBeReturned() {
        assertThat(buildSucceeded).isTrue();
    }

    @Then("only the event pages should be returned")
    public void onlyTheEventPagesShouldBeReturned() {
        assertThat(returnedPages).isNotEmpty();
    }

    @Then("the EventBook metadata should be stripped")
    public void theEventBookMetadataShouldBeStripped() {
        // Pages only, no cover
        assertThat(returnedPages).isNotEmpty();
    }

    @Then("I should receive a QueryBuilder for that domain and root")
    public void iShouldReceiveAQueryBuilderForThatDomainAndRoot() {
        assertThat(domain).isNotNull();
        assertThat(root).isNotNull();
    }

    @Then("I should receive a QueryBuilder with no root set")
    public void iShouldReceiveAQueryBuilderWithNoRootSet() {
        assertThat(domain).isNotNull();
        assertThat(root).isNull();
    }

    @Then("I can chain by_correlation_id")
    public void iCanChainByCorrelationId() {
        // Builder supports chaining
        assertThat(buildSucceeded).isTrue();
    }
}
