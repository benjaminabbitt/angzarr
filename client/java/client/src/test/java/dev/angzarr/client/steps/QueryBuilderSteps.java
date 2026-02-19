package dev.angzarr.client.steps;

import dev.angzarr.Query;
import dev.angzarr.client.Helpers;
import dev.angzarr.client.QueryBuilder;
import io.cucumber.datatable.DataTable;
import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.Map;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for QueryBuilder feature tests.
 *
 * Tests the fluent builder API for constructing Query messages.
 * These are unit tests that don't require a running gRPC server.
 */
public class QueryBuilderSteps {

    private QueryBuilder builder;
    private Query query;
    private UUID testRoot;

    @Before
    public void setup() {
        builder = null;
        query = null;
        testRoot = null;
    }

    // --- Background ---

    @Given("a QueryClient connected to the coordinator")
    public void aQueryClientConnectedToCoordinator() {
        // For build() tests, we don't need an actual client.
    }

    // --- When steps ---

    @When("I build a query using QueryBuilder:")
    public void iBuildAQueryUsingQueryBuilder(DataTable dataTable) {
        Map<String, String> fields = dataTable.asMap(String.class, String.class);

        String domain = fields.get("domain");
        String rootStr = fields.get("root");

        testRoot = rootStr != null ? UUID.fromString(rootStr) : null;

        builder = testRoot != null
            ? new QueryBuilder(null, domain, testRoot)
            : new QueryBuilder(null, domain);

        query = builder.build();
    }

    @When("I build a query with range from {int} to {int}")
    public void iBuildAQueryWithRangeFromTo(int lower, int upper) {
        builder = new QueryBuilder(null, "test")
            .rangeTo(lower, upper);
        query = builder.build();
    }

    @When("I build a query with range from {int}")
    public void iBuildAQueryWithRangeFrom(int lower) {
        builder = new QueryBuilder(null, "test")
            .range(lower);
        query = builder.build();
    }

    @When("I build a query as_of_sequence {int}")
    public void iBuildAQueryAsOfSequence(int seq) {
        builder = new QueryBuilder(null, "test")
            .asOfSequence(seq);
        query = builder.build();
    }

    @When("I build a query as_of_time {string}")
    public void iBuildAQueryAsOfTime(String rfc3339) {
        builder = new QueryBuilder(null, "test")
            .asOfTime(rfc3339);
        query = builder.build();
    }

    @When("I build a query by_correlation_id {string}")
    public void iBuildAQueryByCorrelationId(String correlationId) {
        builder = new QueryBuilder(null, "test")
            .byCorrelationId(correlationId);
        query = builder.build();
    }

    @When("I build a query with_edition {string}")
    public void iBuildAQueryWithEdition(String edition) {
        builder = new QueryBuilder(null, "test")
            .withEdition(edition);
        query = builder.build();
    }

    // --- Then steps ---

    @Then("the resulting Query should have:")
    public void theResultingQueryShouldHave(DataTable dataTable) {
        Map<String, String> expected = dataTable.asMap(String.class, String.class);

        if (expected.containsKey("domain")) {
            assertThat(query.getCover().getDomain()).isEqualTo(expected.get("domain"));
        }
        if (expected.containsKey("root")) {
            UUID expectedRoot = UUID.fromString(expected.get("root"));
            UUID actualRoot = Helpers.protoToUuid(query.getCover().getRoot());
            assertThat(actualRoot).isEqualTo(expectedRoot);
        }
    }

    @Then("the resulting Query should have sequence_range with lower={int} and upper={int}")
    public void theResultingQueryShouldHaveSequenceRangeWithLowerAndUpper(int lower, int upper) {
        assertThat(query.hasRange()).isTrue();
        assertThat(query.getRange().getLower()).isEqualTo(lower);
        assertThat(query.getRange().getUpper()).isEqualTo(upper);
    }

    @Then("the resulting Query should have sequence_range with lower={int} and no upper bound")
    public void theResultingQueryShouldHaveSequenceRangeWithLowerAndNoUpperBound(int lower) {
        assertThat(query.hasRange()).isTrue();
        assertThat(query.getRange().getLower()).isEqualTo(lower);
        // Upper bound is 0 when not set (protobuf default)
        assertThat(query.getRange().getUpper()).isEqualTo(0);
    }

    @Then("the resulting Query should have temporal_query with sequence={int}")
    public void theResultingQueryShouldHaveTemporalQueryWithSequence(int seq) {
        assertThat(query.hasTemporal()).isTrue();
        assertThat(query.getTemporal().getAsOfSequence()).isEqualTo(seq);
    }

    @Then("the resulting Query should have temporal_query with the parsed timestamp")
    public void theResultingQueryShouldHaveTemporalQueryWithTheParsedTimestamp() {
        assertThat(query.hasTemporal()).isTrue();
        assertThat(query.getTemporal().hasAsOfTime()).isTrue();
        // Verify timestamp was parsed (non-zero seconds)
        assertThat(query.getTemporal().getAsOfTime().getSeconds()).isGreaterThan(0);
    }

    @Then("the resulting Query should query by correlation_id {string}")
    public void theResultingQueryShouldQueryByCorrelationId(String correlationId) {
        assertThat(query.getCover().getCorrelationId()).isEqualTo(correlationId);
    }

    @Then("the resulting Query should have edition {string}")
    public void theResultingQueryShouldHaveEdition(String edition) {
        assertThat(query.getCover().hasEdition()).isTrue();
        assertThat(query.getCover().getEdition().getName()).isEqualTo(edition);
    }

    // --- Skipped scenarios that need real gRPC ---

    @Given("an aggregate {string} with root {string} has {int} events")
    public void anAggregateWithRootHasEvents(String domain, String root, int eventCount) {
        // Skip - requires real gRPC server
        org.junit.jupiter.api.Assumptions.assumeTrue(false,
            "Skipping: requires running gRPC server");
    }

    @When("I use QueryBuilder to get_event_book for that root")
    public void iUseQueryBuilderToGetEventBookForThatRoot() {
        // Skip - requires real gRPC server
    }

    @When("I use QueryBuilder to get_pages for that root")
    public void iUseQueryBuilderToGetPagesForThatRoot() {
        // Skip - requires real gRPC server
    }

    @Then("I should receive an EventBook with {int} pages")
    public void iShouldReceiveAnEventBookWithPages(int count) {
        // Skip - requires real gRPC server
    }

    @Then("the EventBook should have the correct domain and root")
    public void theEventBookShouldHaveTheCorrectDomainAndRoot() {
        // Skip - requires real gRPC server
    }

    @Then("I should receive a list of {int} EventPages")
    public void iShouldReceiveAListOfEventPages(int count) {
        // Skip - requires real gRPC server
    }
}
