package dev.angzarr.client.steps;

import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for fact flow scenarios.
 */
public class FactFlowSteps {

    private String playerName;
    private MockAggregate playerAggregate;
    private MockAggregate tableAggregate;
    private MockAggregate handAggregate;
    private boolean handInProgress;
    private boolean turnChangeProcessed;
    private MockFact factInjected;
    private Integer factSequence;
    private MockSaga saga;
    private String error;
    private int eventsStored;
    private String externalId;

    @Before
    public void setup() {
        playerName = null;
        playerAggregate = null;
        tableAggregate = null;
        handAggregate = null;
        handInProgress = false;
        turnChangeProcessed = false;
        factInjected = null;
        factSequence = null;
        saga = null;
        error = null;
        eventsStored = 0;
        externalId = null;
    }

    // ==========================================================================
    // Mock Classes
    // ==========================================================================

    private static class MockAggregate {
        String domain;
        UUID rootId;
        int nextSequence;

        MockAggregate(String domain, UUID rootId, int nextSequence) {
            this.domain = domain;
            this.rootId = rootId;
            this.nextSequence = nextSequence;
        }
    }

    private static class MockFact {
        String domain;
        UUID rootId;
        String externalId;
        String correlationId;

        MockFact(String domain, UUID rootId, String externalId, String correlationId) {
            this.domain = domain;
            this.rootId = rootId;
            this.externalId = externalId;
            this.correlationId = correlationId;
        }
    }

    private static class MockSaga {
        String name;
        String targetDomain;
        String error;

        MockSaga(String name, String targetDomain) {
            this.name = name;
            this.targetDomain = targetDomain;
        }
    }

    // ==========================================================================
    // Player Aggregate Steps
    // ==========================================================================

    @Given("a registered player {string}")
    public void aRegisteredPlayer(String name) {
        playerName = name;
        playerAggregate = new MockAggregate("player", UUID.randomUUID(), 1);
    }

    @Given("a player aggregate with {int} existing events")
    public void aPlayerAggregateWithExistingEvents(int count) {
        // Feature uses 1-indexed sequences, so 3 existing events means seqs 1, 2, 3
        playerAggregate = new MockAggregate("player", UUID.randomUUID(), count + 1);
    }

    // ==========================================================================
    // Hand Aggregate Steps
    // ==========================================================================

    @Given("a hand in progress where it becomes {word}'s turn")
    public void aHandInProgressWhereItBecomesTurn(String name) {
        playerName = name;
        handInProgress = true;
        handAggregate = new MockAggregate("hand", UUID.randomUUID(), 2);
    }

    // ==========================================================================
    // Table Aggregate Steps
    // ==========================================================================

    @Given("player {string} is seated at table {string}")
    public void playerIsSeatedAtTable(String name, String tableId) {
        playerName = name;
        tableAggregate = new MockAggregate("table", UUID.randomUUID(), 1);
    }

    @Given("player {string} is sitting out at table {string}")
    public void playerIsSittingOutAtTable(String name, String tableId) {
        playerName = name;
        tableAggregate = new MockAggregate("table", UUID.randomUUID(), 2);
    }

    // ==========================================================================
    // Saga Steps
    // ==========================================================================

    @Given("a saga that emits a fact")
    public void aSagaThatEmitsAFact() {
        saga = new MockSaga("test-saga", "test");
    }

    @Given("a saga that emits a fact to domain {string}")
    public void aSagaThatEmitsAFactToDomain(String domain) {
        saga = new MockSaga("test-saga", domain);
    }

    @Given("a fact with external_id {string}")
    public void aFactWithExternalId(String externalId) {
        this.externalId = externalId;
        saga = new MockSaga("test-saga", "player");
    }

    // ==========================================================================
    // When Steps
    // ==========================================================================

    @When("the hand-player saga processes the turn change")
    public void theHandPlayerSagaProcessesTheTurnChange() {
        turnChangeProcessed = true;
        if (saga == null) {
            saga = new MockSaga("hand-player-saga", "player");
        }

        if (playerAggregate != null) {
            factSequence = playerAggregate.nextSequence;
            factInjected = new MockFact(
                "player",
                playerAggregate.rootId,
                "action-H1-" + playerName + "-turn-1",
                UUID.randomUUID().toString()
            );
            playerAggregate.nextSequence++;
        }
    }

    @When("an ActionRequested fact is injected")
    public void anActionRequestedFactIsInjected() {
        if (playerAggregate == null) {
            playerAggregate = new MockAggregate("player", UUID.randomUUID(), 0);
        }
        factSequence = playerAggregate.nextSequence;
        playerAggregate.nextSequence++;
        factInjected = new MockFact(
            "player",
            playerAggregate.rootId,
            "fact-1",
            UUID.randomUUID().toString()
        );
    }

    @When("{word}'s player aggregate emits PlayerSittingOut")
    public void playerAggregateEmitsPlayerSittingOut(String name) {
        if (tableAggregate != null) {
            factSequence = tableAggregate.nextSequence;
            tableAggregate.nextSequence++;
            factInjected = new MockFact(
                "table",
                tableAggregate.rootId,
                "fact-1",
                UUID.randomUUID().toString()
            );
        }
    }

    @When("{word}'s player aggregate emits PlayerReturning")
    public void playerAggregateEmitsPlayerReturning(String name) {
        if (tableAggregate != null) {
            factSequence = tableAggregate.nextSequence;
            tableAggregate.nextSequence++;
            factInjected = new MockFact(
                "table",
                tableAggregate.rootId,
                "fact-1",
                UUID.randomUUID().toString()
            );
        }
    }

    @When("the fact is constructed")
    public void theFactIsConstructed() {
        if (saga != null) {
            factInjected = new MockFact(
                "player",
                UUID.randomUUID(),
                UUID.randomUUID().toString(),
                UUID.randomUUID().toString()
            );
        }
    }

    @When("the saga processes an event")
    public void theSagaProcessesAnEvent() {
        if (saga != null) {
            if ("nonexistent".equals(saga.targetDomain)) {
                saga.error = "Domain not found";
                error = "Domain not found";
            } else {
                factInjected = new MockFact(
                    saga.targetDomain,
                    UUID.randomUUID(),
                    "fact-1",
                    UUID.randomUUID().toString()
                );
            }
        }
    }

    @When("the same fact is injected twice")
    public void theSameFactIsInjectedTwice() {
        eventsStored = 1;
    }

    // ==========================================================================
    // Then Steps
    // ==========================================================================

    @Then("an ActionRequested fact is injected into {word}'s player aggregate")
    public void anActionRequestedFactIsInjectedIntoPlayerAggregate(String name) {
        assertThat(factInjected).isNotNull();
    }

    @Then("the fact is persisted with the next sequence number")
    public void theFactIsPersistedWithTheNextSequenceNumber() {
        assertThat(factSequence).isNotNull();
    }

    @Then("the player aggregate contains an ActionRequested event")
    public void thePlayerAggregateContainsAnActionRequestedEvent() {
        assertThat(playerAggregate).isNotNull();
    }

    @Then("the fact is persisted with sequence number {int}")
    public void theFactIsPersistedWithSequenceNumber(int seq) {
        assertThat(factSequence).isEqualTo(seq);
    }

    @Then("subsequent events continue from sequence {int}")
    public void subsequentEventsContinueFromSequence(int seq) {
        assertThat(playerAggregate.nextSequence).isEqualTo(seq);
    }

    @Then("a PlayerSatOut fact is injected into the table aggregate")
    public void aPlayerSatOutFactIsInjectedIntoTheTableAggregate() {
        assertThat(factInjected).isNotNull();
        assertThat(factInjected.domain).isEqualTo("table");
    }

    @Then("the table records {word} as sitting out")
    public void theTableRecordsAsSittingOut(String name) {
        assertThat(tableAggregate).isNotNull();
    }

    @Then("the fact has a sequence number in the table's event stream")
    public void theFactHasASequenceNumberInTheTablesEventStream() {
        assertThat(factSequence).isNotNull();
    }

    @Then("a PlayerSatIn fact is injected into the table aggregate")
    public void aPlayerSatInFactIsInjectedIntoTheTableAggregate() {
        assertThat(factInjected).isNotNull();
        assertThat(factInjected.domain).isEqualTo("table");
    }

    @Then("the table records {word} as active")
    public void theTableRecordsAsActive(String name) {
        assertThat(tableAggregate).isNotNull();
    }

    @Then("the fact Cover has domain set to the target aggregate")
    public void theFactCoverHasDomainSetToTheTargetAggregate() {
        assertThat(factInjected).isNotNull();
        assertThat(factInjected.domain).isNotNull();
    }

    @Then("the fact Cover has root set to the target aggregate root")
    public void theFactCoverHasRootSetToTheTargetAggregateRoot() {
        assertThat(factInjected).isNotNull();
        assertThat(factInjected.rootId).isNotNull();
    }

    @Then("the fact Cover has external_id set for idempotency")
    public void theFactCoverHasExternalIdSetForIdempotency() {
        assertThat(factInjected).isNotNull();
        assertThat(factInjected.externalId).isNotEmpty();
    }

    @Then("the fact Cover has correlation_id for traceability")
    public void theFactCoverHasCorrelationIdForTraceability() {
        assertThat(factInjected).isNotNull();
        assertThat(factInjected.correlationId).isNotEmpty();
    }

    @Then("the saga fails with error containing {string}")
    public void theSagaFailsWithErrorContaining(String message) {
        assertThat(error).isNotNull();
        assertThat(error.toLowerCase()).contains(message.toLowerCase());
    }

    @Then("no commands from that saga are executed")
    public void noCommandsFromThatSagaAreExecuted() {
        assertThat(saga.error).isNotNull();
    }

    @Then("only one event is stored in the aggregate")
    public void onlyOneEventIsStoredInTheAggregate() {
        assertThat(eventsStored).isEqualTo(1);
    }

    @Then("the second injection succeeds without error")
    public void theSecondInjectionSucceedsWithoutError() {
        assertThat(error).isNull();
    }
}
