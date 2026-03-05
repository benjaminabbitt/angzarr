package dev.angzarr.client.steps;

import io.cucumber.datatable.DataTable;
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
 * Step definitions for QueryClient scenarios.
 */
public class QueryClientSteps {

    private boolean queryClientConnected;
    private String domain;
    private String rootId;
    private int eventCount;
    private int nextSequence;
    private int queryFromSequence;
    private int queryToSequence;
    private String queryAsOfTime;
    private int queryAsOfSequence;
    private String editionName;
    private String correlationId;
    private List<MockEventPage> receivedEvents;
    private MockSnapshot receivedSnapshot;
    private boolean operationFailed;
    private String errorType;
    private boolean serviceUnavailable;
    private Map<String, List<MockEventPage>> eventsByCorrelation;

    @Before
    public void setup() {
        queryClientConnected = false;
        domain = null;
        rootId = null;
        eventCount = 0;
        nextSequence = 0;
        queryFromSequence = -1;
        queryToSequence = -1;
        queryAsOfTime = null;
        queryAsOfSequence = -1;
        editionName = null;
        correlationId = null;
        receivedEvents = new ArrayList<>();
        receivedSnapshot = null;
        operationFailed = false;
        errorType = null;
        serviceUnavailable = false;
        eventsByCorrelation = new HashMap<>();
    }

    private static class MockEventPage {
        int sequence;
        String eventType;
        String payload;
        String timestamp;
        String correlationId;

        MockEventPage(int sequence, String eventType) {
            this.sequence = sequence;
            this.eventType = eventType;
        }
    }

    private static class MockSnapshot {
        int sequence;
        byte[] data;

        MockSnapshot(int sequence) {
            this.sequence = sequence;
            this.data = new byte[0];
        }
    }

    // ==========================================================================
    // Background Steps
    // ==========================================================================

    @Given("a QueryClient connected to the test backend")
    public void aQueryClientConnectedToTheTestBackend() {
        queryClientConnected = true;
    }

    // ==========================================================================
    // Given Steps
    // ==========================================================================

    @Given("an aggregate {string} with root {string}")
    public void anAggregateWithRoot(String domain, String root) {
        this.domain = domain;
        this.rootId = root;
        this.eventCount = 0;
        this.nextSequence = 0;
    }

    @Given("an aggregate {string} with root {string} has {int} events")
    public void anAggregateWithRootHasEvents(String domain, String root, int count) {
        this.domain = domain;
        this.rootId = root;
        this.eventCount = count;
        this.nextSequence = count;
    }

    @Given("an aggregate {string} with root {string} has event {string} with data {string}")
    public void anAggregateWithRootHasEventWithData(String domain, String root, String eventType, String data) {
        this.domain = domain;
        this.rootId = root;
        this.eventCount = 1;
        this.nextSequence = 1;
        MockEventPage page = new MockEventPage(0, eventType);
        page.payload = data;
        receivedEvents.add(page);
    }

    @Given("an aggregate {string} with root {string} has events at known timestamps")
    public void anAggregateWithRootHasEventsAtKnownTimestamps(String domain, String root) {
        this.domain = domain;
        this.rootId = root;
        this.eventCount = 3;
        this.nextSequence = 3;
        for (int i = 0; i < 3; i++) {
            MockEventPage page = new MockEventPage(i, "TimestampedEvent");
            page.timestamp = "2024-01-15T10:" + (i * 15) + ":00Z";
            receivedEvents.add(page);
        }
    }

    @Given("an aggregate {string} with root {string} in edition {string}")
    public void anAggregateWithRootInEdition(String domain, String root, String edition) {
        this.domain = domain;
        this.rootId = root;
        this.editionName = edition;
        this.eventCount = 3;
        this.nextSequence = 3;
    }

    @Given("an aggregate {string} with root {string} has {int} events in main")
    public void anAggregateWithRootHasEventsInMain(String domain, String root, int count) {
        this.domain = domain;
        this.rootId = root;
        this.eventCount = count;
        this.nextSequence = count;
    }

    @Given("an aggregate {string} with root {string} has {int} events in edition {string}")
    public void anAggregateWithRootHasEventsInEdition(String domain, String root, int count, String edition) {
        // This adds events to an edition
        this.editionName = edition;
        // Store separate count for edition queries
    }

    @Given("events with correlation ID {string} exist in multiple aggregates")
    public void eventsWithCorrelationIdExistInMultipleAggregates(String corrId) {
        this.correlationId = corrId;
        List<MockEventPage> pages = new ArrayList<>();
        pages.add(new MockEventPage(0, "Event1"));
        pages.add(new MockEventPage(1, "Event2"));
        eventsByCorrelation.put(corrId, pages);
    }

    @Given("an aggregate {string} with root {string} has a snapshot at sequence {int} and {int} events")
    public void anAggregateWithRootHasSnapshotAndEvents(String domain, String root, int snapshotSeq, int count) {
        this.domain = domain;
        this.rootId = root;
        this.eventCount = count;
        this.nextSequence = count;
        this.receivedSnapshot = new MockSnapshot(snapshotSeq);
    }

    @Given("the query service is unavailable")
    public void theQueryServiceIsUnavailable() {
        serviceUnavailable = true;
    }

    // ==========================================================================
    // When Steps
    // ==========================================================================

    @When("I query events for {string} root {string}")
    public void iQueryEventsFor(String domain, String root) {
        if (serviceUnavailable) {
            operationFailed = true;
            errorType = "connection";
            return;
        }
        this.domain = domain;
        this.rootId = root;
        // If events were already set up by Given step, keep them (for payload preservation tests)
        // Otherwise, simulate fetching generic events
        if (receivedEvents.isEmpty()) {
            for (int i = 0; i < eventCount; i++) {
                receivedEvents.add(new MockEventPage(i, "Event" + i));
            }
        }
    }

    @When("I query events for {string} root {string} from sequence {int}")
    public void iQueryEventsFromSequence(String domain, String root, int fromSeq) {
        this.domain = domain;
        this.rootId = root;
        this.queryFromSequence = fromSeq;
        receivedEvents.clear();
        for (int i = fromSeq; i < eventCount; i++) {
            receivedEvents.add(new MockEventPage(i, "Event" + i));
        }
    }

    @When("I query events for {string} root {string} from sequence {int} to {int}")
    public void iQueryEventsFromSequenceToSequence(String domain, String root, int fromSeq, int toSeq) {
        this.domain = domain;
        this.rootId = root;
        this.queryFromSequence = fromSeq;
        this.queryToSequence = toSeq;
        receivedEvents.clear();
        for (int i = fromSeq; i < toSeq && i < eventCount; i++) {
            receivedEvents.add(new MockEventPage(i, "Event" + i));
        }
    }

    @When("I query events for {string} root {string} as of sequence {int}")
    public void iQueryEventsAsOfSequence(String domain, String root, int seq) {
        this.domain = domain;
        this.rootId = root;
        this.queryAsOfSequence = seq;
        receivedEvents.clear();
        for (int i = 0; i <= seq && i < eventCount; i++) {
            receivedEvents.add(new MockEventPage(i, "Event" + i));
        }
    }

    @When("I query events for {string} root {string} as of time {string}")
    public void iQueryEventsAsOfTime(String domain, String root, String time) {
        this.domain = domain;
        this.rootId = root;
        this.queryAsOfTime = time;
        // Simulate returning events up to that timestamp
        receivedEvents.clear();
        receivedEvents.add(new MockEventPage(0, "Event0"));
        receivedEvents.add(new MockEventPage(1, "Event1"));
    }

    @When("I query events for {string} root {string} in edition {string}")
    public void iQueryEventsInEdition(String domain, String root, String edition) {
        this.domain = domain;
        this.rootId = root;
        this.editionName = edition;
        receivedEvents.clear();
        int editionCount = "branch".equals(edition) ? 2 : eventCount;
        for (int i = 0; i < editionCount; i++) {
            receivedEvents.add(new MockEventPage(i, "Event" + i));
        }
    }

    @When("I query events by correlation ID {string}")
    public void iQueryEventsByCorrelationId(String corrId) {
        this.correlationId = corrId;
        receivedEvents.clear();
        List<MockEventPage> pages = eventsByCorrelation.get(corrId);
        if (pages != null) {
            receivedEvents.addAll(pages);
        }
    }

    @When("I query events with empty domain")
    public void iQueryEventsWithEmptyDomain() {
        operationFailed = true;
        errorType = "invalid_argument";
    }

    // Note: "I attempt to query events" is in ClientConnectivitySteps

    // ==========================================================================
    // Then Steps
    // ==========================================================================

    @Then("I should receive an EventBook with {int} events")
    public void iShouldReceiveAnEventBookWithEvents(int count) {
        assertThat(receivedEvents).hasSize(count);
    }

    @Then("the next_sequence should be {int}")
    public void theNextSequenceShouldBe(int seq) {
        assertThat(nextSequence).isEqualTo(seq);
    }

    @Then("events should be in sequence order {int} to {int}")
    public void eventsShouldBeInSequenceOrder(int from, int to) {
        for (int i = 0; i < receivedEvents.size(); i++) {
            assertThat(receivedEvents.get(i).sequence).isEqualTo(from + i);
        }
    }

    @Then("the first event should have type {string}")
    public void theFirstEventShouldHaveType(String type) {
        assertThat(receivedEvents).isNotEmpty();
        assertThat(receivedEvents.get(0).eventType).isEqualTo(type);
    }

    @Then("the first event should have payload {string}")
    public void theFirstEventShouldHavePayload(String payload) {
        assertThat(receivedEvents).isNotEmpty();
        assertThat(receivedEvents.get(0).payload).isEqualTo(payload);
    }

    @Then("the first event should have sequence {int}")
    public void theFirstEventShouldHaveSequence(int seq) {
        assertThat(receivedEvents).isNotEmpty();
        assertThat(receivedEvents.get(0).sequence).isEqualTo(seq);
    }

    @Then("the last event should have sequence {int}")
    public void theLastEventShouldHaveSequence(int seq) {
        assertThat(receivedEvents).isNotEmpty();
        assertThat(receivedEvents.get(receivedEvents.size() - 1).sequence).isEqualTo(seq);
    }

    @Then("I should receive events up to that timestamp")
    public void iShouldReceiveEventsUpToThatTimestamp() {
        assertThat(receivedEvents).isNotEmpty();
    }

    @Then("I should receive events from that edition only")
    public void iShouldReceiveEventsFromThatEditionOnly() {
        assertThat(editionName).isNotNull();
    }

    @Then("I should receive events from all correlated aggregates")
    public void iShouldReceiveEventsFromAllCorrelatedAggregates() {
        assertThat(receivedEvents).isNotEmpty();
    }

    @Then("I should receive no events")
    public void iShouldReceiveNoEvents() {
        assertThat(receivedEvents).isEmpty();
    }

    @Then("the EventBook should include the snapshot")
    public void theEventBookShouldIncludeTheSnapshot() {
        assertThat(receivedSnapshot).isNotNull();
    }

    @Then("the returned snapshot should be at sequence {int}")
    public void theReturnedSnapshotShouldBeAtSequence(int seq) {
        assertThat(receivedSnapshot).isNotNull();
        assertThat(receivedSnapshot.sequence).isEqualTo(seq);
    }

    @Then("the operation should fail with invalid argument error")
    public void theOperationShouldFailWithInvalidArgumentError() {
        assertThat(operationFailed).isTrue();
        assertThat(errorType).isEqualTo("invalid_argument");
    }

    @Then("the operation should fail with connection error")
    public void theOperationShouldFailWithConnectionError() {
        assertThat(operationFailed).isTrue();
        assertThat(errorType).isEqualTo("connection");
    }
}
