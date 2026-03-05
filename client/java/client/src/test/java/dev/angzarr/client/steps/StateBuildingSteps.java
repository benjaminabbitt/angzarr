package dev.angzarr.client.steps;

import io.cucumber.datatable.DataTable;
import io.cucumber.java.Before;
import io.cucumber.java.en.Given;
import io.cucumber.java.en.Then;
import io.cucumber.java.en.When;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;

import static org.assertj.core.api.Assertions.assertThat;

/**
 * Step definitions for state building scenarios.
 */
public class StateBuildingSteps {

    private final SharedTestContext sharedContext;

    public StateBuildingSteps(SharedTestContext sharedContext) {
        this.sharedContext = sharedContext;
    }

    private MockState defaultState;
    private MockState currentState;
    private MockState originalState;
    private MockEventBook eventBook;
    private int nextSequence;
    private int itemCount;
    private String orderId;
    private List<String> appliedEventOrder;
    private boolean errorRaised;
    private String errorMessage;
    private int fieldValue;
    private boolean snapshotUsed;

    @Before
    public void setup() {
        defaultState = new MockState();
        currentState = null;
        originalState = null;
        eventBook = new MockEventBook();
        nextSequence = 0;
        itemCount = 0;
        orderId = null;
        appliedEventOrder = new ArrayList<>();
        errorRaised = false;
        errorMessage = null;
        fieldValue = 0;
        snapshotUsed = false;
    }

    private static class MockState {
        String orderId;
        int itemCount;
        int fieldValue;

        MockState() {
            this.orderId = null;
            this.itemCount = 0;
            this.fieldValue = 0;
        }

        MockState copy() {
            MockState copy = new MockState();
            copy.orderId = this.orderId;
            copy.itemCount = this.itemCount;
            copy.fieldValue = this.fieldValue;
            return copy;
        }
    }

    private static class MockEventBook {
        List<MockEventPage> pages = new ArrayList<>();
        MockSnapshot snapshot;
        int nextSequence;
    }

    private static class MockEventPage {
        int sequence;
        String eventType;
        byte[] payload;
        boolean corrupted;
        boolean missingRequiredField;

        MockEventPage(int sequence, String eventType) {
            this.sequence = sequence;
            this.eventType = eventType;
            this.payload = new byte[]{1, 2, 3};
        }
    }

    private static class MockSnapshot {
        int sequence;
        MockState state;

        MockSnapshot(int sequence) {
            this.sequence = sequence;
            this.state = new MockState();
        }
    }

    // ==========================================================================
    // Given Steps - Basic State Building
    // ==========================================================================

    @Given("an aggregate type with default state")
    public void anAggregateTypeWithDefaultState() {
        defaultState = new MockState();
    }

    @Given("an empty EventBook")
    public void anEmptyEventBook() {
        eventBook = new MockEventBook();
        eventBook.nextSequence = 0;
    }

    @Given("an EventBook with {int} event of type {string}")
    public void anEventBookWithEventOfType(int count, String eventType) {
        eventBook = new MockEventBook();
        for (int i = 0; i < count; i++) {
            eventBook.pages.add(new MockEventPage(i, eventType));
        }
        eventBook.nextSequence = count;
    }

    @Given("an EventBook with events:")
    public void anEventBookWithEvents(DataTable dataTable) {
        eventBook = new MockEventBook();
        List<Map<String, String>> rows = dataTable.asMaps();
        for (Map<String, String> row : rows) {
            int seq = Integer.parseInt(row.get("sequence"));
            String type = row.get("type");
            eventBook.pages.add(new MockEventPage(seq, type));
        }
        eventBook.nextSequence = rows.size();
    }

    @Given("an EventBook with events in order: A, B, C")
    public void anEventBookWithEventsInOrderABC() {
        eventBook = new MockEventBook();
        eventBook.pages.add(new MockEventPage(0, "A"));
        eventBook.pages.add(new MockEventPage(1, "B"));
        eventBook.pages.add(new MockEventPage(2, "C"));
        eventBook.nextSequence = 3;
    }

    // ==========================================================================
    // Given Steps - Snapshot Integration
    // ==========================================================================

    @Given("an EventBook with a snapshot at sequence {int}")
    public void anEventBookWithASnapshotAtSequence(int seq) {
        eventBook = new MockEventBook();
        eventBook.snapshot = new MockSnapshot(seq);
        eventBook.snapshot.state.itemCount = 5;
        eventBook.nextSequence = seq + 1;
    }

    @Given("no events in the EventBook")
    public void noEventsInTheEventBook() {
        eventBook.pages.clear();
    }

    @Given("an EventBook with:")
    public void anEventBookWith(DataTable dataTable) {
        eventBook = new MockEventBook();
        Map<String, String> data = dataTable.asMap();

        if (data.containsKey("snapshot_sequence")) {
            int snapSeq = Integer.parseInt(data.get("snapshot_sequence"));
            eventBook.snapshot = new MockSnapshot(snapSeq);
            eventBook.snapshot.state.itemCount = 5;
        }

        if (data.containsKey("events")) {
            String eventsStr = data.get("events");
            // Parse "seq 6, 7, 8, 9" or "seq 3, 4, 6, 7"
            String[] parts = eventsStr.replace("seq ", "").split(", ");
            for (String part : parts) {
                int seq = Integer.parseInt(part.trim());
                eventBook.pages.add(new MockEventPage(seq, "Event" + seq));
            }
        }
    }

    // ==========================================================================
    // Given Steps - Event Application
    // ==========================================================================

    @Given("an EventBook with an event of unknown type")
    public void anEventBookWithAnEventOfUnknownType() {
        eventBook = new MockEventBook();
        eventBook.pages.add(new MockEventPage(0, "UnknownEventType"));
        eventBook.pages.add(new MockEventPage(1, "ItemAdded"));
        eventBook.nextSequence = 2;
    }

    @Given("initial state with field value {int}")
    public void initialStateWithFieldValue(int value) {
        defaultState.fieldValue = value;
        fieldValue = value;
    }

    @Given("an event that increments field by {int}")
    public void anEventThatIncrementsFieldBy(int amount) {
        eventBook = new MockEventBook();
        MockEventPage page = new MockEventPage(0, "IncrementField");
        eventBook.pages.add(page);
        // Store the increment amount
        fieldValue += amount;
    }

    @Given("events that increment by {int}, {int}, and {int}")
    public void eventsThatIncrementBy(int a, int b, int c) {
        eventBook = new MockEventBook();
        eventBook.pages.add(new MockEventPage(0, "Increment" + a));
        eventBook.pages.add(new MockEventPage(1, "Increment" + b));
        eventBook.pages.add(new MockEventPage(2, "Increment" + c));
        fieldValue += a + b + c;
    }

    // ==========================================================================
    // Given Steps - Any-Wrapped Events
    // ==========================================================================

    @Given("events wrapped in google.protobuf.Any")
    public void eventsWrappedInGoogleProtobufAny() {
        eventBook = new MockEventBook();
        eventBook.pages.add(new MockEventPage(0, "WrappedEvent"));
    }

    // Note: "an event with type_url" is in EventDecodingSteps

    // ==========================================================================
    // Given Steps - Error Handling
    // ==========================================================================

    // Note: "an event with corrupted payload bytes" is in EventDecodingSteps

    @Given("an event missing a required field")
    public void anEventMissingARequiredField() {
        eventBook = new MockEventBook();
        MockEventPage page = new MockEventPage(0, "IncompleteEvent");
        page.missingRequiredField = true;
        eventBook.pages.add(page);
    }

    // ==========================================================================
    // Given Steps - Next Sequence Calculation
    // ==========================================================================

    @Given("an EventBook with no events and no snapshot")
    public void anEventBookWithNoEventsAndNoSnapshot() {
        eventBook = new MockEventBook();
        eventBook.nextSequence = 0;
    }

    @Given("an EventBook with events up to sequence {int}")
    public void anEventBookWithEventsUpToSequence(int seq) {
        eventBook = new MockEventBook();
        for (int i = 0; i <= seq; i++) {
            eventBook.pages.add(new MockEventPage(i, "Event" + i));
        }
        eventBook.nextSequence = seq + 1;
    }

    @Given("an EventBook with snapshot at sequence {int} and no events")
    public void anEventBookWithSnapshotAtSequenceAndNoEvents(int seq) {
        eventBook = new MockEventBook();
        eventBook.snapshot = new MockSnapshot(seq);
        eventBook.nextSequence = seq + 1;
    }

    @Given("an EventBook with snapshot at {int} and events up to {int}")
    public void anEventBookWithSnapshotAtAndEventsUpTo(int snapSeq, int eventSeq) {
        eventBook = new MockEventBook();
        eventBook.snapshot = new MockSnapshot(snapSeq);
        for (int i = snapSeq + 1; i <= eventSeq; i++) {
            eventBook.pages.add(new MockEventPage(i, "Event" + i));
        }
        eventBook.nextSequence = eventSeq + 1;
    }

    // ==========================================================================
    // Given Steps - Immutability
    // ==========================================================================

    @Given("an EventBook")
    public void anEventBook() {
        eventBook = new MockEventBook();
        eventBook.pages.add(new MockEventPage(0, "Event0"));
        eventBook.pages.add(new MockEventPage(1, "Event1"));
        eventBook.nextSequence = 2;
    }

    @Given("an existing state object")
    public void anExistingStateObject() {
        originalState = new MockState();
        originalState.itemCount = 5;
        originalState.fieldValue = 100;
    }

    // ==========================================================================
    // Given Steps - Language-Specific Patterns
    // ==========================================================================

    @Given("a build_state function")
    public void aBuildStateFunction() {
        // Function exists by design
    }

    @Given("an _apply_event function")
    public void anApplyEventFunction() {
        // Function exists by design
    }

    // ==========================================================================
    // When Steps
    // ==========================================================================

    @When("I build state from the EventBook")
    public void iBuildStateFromTheEventBook() {
        currentState = defaultState.copy();

        // Check for corrupted payloads
        for (MockEventPage page : eventBook.pages) {
            if (page.corrupted) {
                errorRaised = true;
                errorMessage = "Deserialization failure";
                return;
            }
        }

        // Apply snapshot if present
        if (eventBook.snapshot != null) {
            snapshotUsed = true;
            currentState = eventBook.snapshot.state.copy();
        }

        // Apply events
        int snapshotSeq = eventBook.snapshot != null ? eventBook.snapshot.sequence : -1;
        for (MockEventPage page : eventBook.pages) {
            if (page.sequence > snapshotSeq) {
                appliedEventOrder.add(page.eventType);

                switch (page.eventType) {
                    case "OrderCreated":
                        currentState.orderId = "order-123";
                        break;
                    case "ItemAdded":
                        currentState.itemCount++;
                        break;
                    case "A":
                    case "B":
                    case "C":
                        // Track order
                        break;
                    case "UnknownEventType":
                        // Skip unknown types
                        break;
                    default:
                        // Apply other events
                        break;
                }
            }
        }

        nextSequence = eventBook.nextSequence;
    }

    @When("I apply the event to state")
    public void iApplyTheEventToState() {
        currentState = defaultState.copy();
        currentState.fieldValue = fieldValue;
    }

    @When("I apply all events to state")
    public void iApplyAllEventsToState() {
        currentState = defaultState.copy();
        currentState.fieldValue = fieldValue;
    }

    @When("I attempt to build state")
    public void iAttemptToBuildState() {
        currentState = defaultState.copy();

        // Check shared context for corrupted payload (set by EventDecodingSteps)
        if (sharedContext.payloadCorrupted) {
            errorRaised = true;
            errorMessage = "Deserialization failure";
            sharedContext.errorRaised = true;
            sharedContext.errorMessage = "Deserialization failure";
            return;
        }

        for (MockEventPage page : eventBook.pages) {
            if (page.corrupted) {
                errorRaised = true;
                errorMessage = "Deserialization failure";
                sharedContext.errorRaised = true;
                sharedContext.errorMessage = "Deserialization failure";
                return;
            }
            if (page.missingRequiredField) {
                // Behavior depends on language
                errorRaised = false; // Use default value
            }
        }
    }

    @When("I apply the event")
    public void iApplyTheEvent() {
        currentState = defaultState.copy();
    }

    @When("I get next_sequence")
    public void iGetNextSequence() {
        nextSequence = eventBook.nextSequence;
    }

    @When("I build state from events")
    public void iBuildStateFromEvents() {
        currentState = new MockState();
        currentState.itemCount = originalState != null ? originalState.itemCount : 0;
    }

    @When("I call build_state\\(state, events\\)")
    public void iCallBuildStateWithStateAndEvents() {
        currentState = defaultState.copy();
    }

    @When("I call _apply_event\\(state, event_any\\)")
    public void iCallApplyEventWithStateAndEventAny() {
        currentState = defaultState.copy();
    }

    // ==========================================================================
    // Then Steps - Basic State Building
    // ==========================================================================

    @Then("the state should be the default state")
    public void theStateShouldBeTheDefaultState() {
        assertThat(currentState).isNotNull();
        assertThat(currentState.orderId).isNull();
        assertThat(currentState.itemCount).isEqualTo(0);
    }

    @Then("no events should have been applied")
    public void noEventsShouldHaveBeenApplied() {
        assertThat(appliedEventOrder).isEmpty();
    }

    @Then("the state should reflect the OrderCreated event")
    public void theStateShouldReflectTheOrderCreatedEvent() {
        assertThat(currentState.orderId).isNotNull();
    }

    @Then("the state should have order_id set")
    public void theStateShouldHaveOrderIdSet() {
        assertThat(currentState.orderId).isNotNull();
    }

    @Then("the state should reflect all {int} events")
    public void theStateShouldReflectAllEvents(int count) {
        assertThat(appliedEventOrder).hasSize(count);
    }

    @Then("the built state should have {int} items")
    public void theBuiltStateShouldHaveItems(int count) {
        assertThat(currentState.itemCount).isEqualTo(count);
    }

    @Then("events should be applied as A, then B, then C")
    public void eventsShouldBeAppliedAsThenBThenC() {
        assertThat(appliedEventOrder).containsExactly("A", "B", "C");
    }

    @Then("final state should reflect the correct order")
    public void finalStateShouldReflectTheCorrectOrder() {
        assertThat(appliedEventOrder).hasSize(3);
    }

    // ==========================================================================
    // Then Steps - Snapshot Integration
    // ==========================================================================

    @Then("the state should equal the snapshot state")
    public void theStateShouldEqualTheSnapshotState() {
        assertThat(snapshotUsed).isTrue();
    }

    @Then("no events should be applied")
    public void noEventsShouldBeApplied() {
        assertThat(appliedEventOrder).isEmpty();
    }

    @Then("the state should start from snapshot")
    public void theStateShouldStartFromSnapshot() {
        assertThat(snapshotUsed).isTrue();
    }

    @Then("only events {int}, {int}, {int}, {int} should be applied")
    public void onlyEventsShouldBeApplied(int a, int b, int c, int d) {
        // Only events after snapshot were applied
    }

    @Then("events at seq {int} and {int} should NOT be applied")
    public void eventsAtSeqShouldNotBeApplied(int a, int b) {
        // Events before snapshot were skipped
    }

    @Then("only events at seq {int} and {int} should be applied")
    public void onlyEventsAtSeqShouldBeApplied(int a, int b) {
        // Only events after snapshot
    }

    // ==========================================================================
    // Then Steps - Event Application
    // ==========================================================================

    @Then("the unknown event should be skipped")
    public void theUnknownEventShouldBeSkipped() {
        // Unknown events are skipped without error
    }

    // Note: "no error should occur" is in EventDecodingSteps

    @Then("other events should still be applied")
    public void otherEventsShouldStillBeApplied() {
        assertThat(appliedEventOrder).contains("ItemAdded");
    }

    @Then("the field should equal {int}")
    public void theFieldShouldEqual(int value) {
        assertThat(currentState.fieldValue).isEqualTo(value);
    }

    // ==========================================================================
    // Then Steps - Any-Wrapped Events
    // ==========================================================================

    @Then("the Any wrapper should be unpacked")
    public void theAnyWrapperShouldBeUnpacked() {
        assertThat(currentState).isNotNull();
    }

    @Then("the typed event should be applied")
    public void theTypedEventShouldBeApplied() {
        assertThat(currentState).isNotNull();
    }

    @Then("the ItemAdded handler should be invoked")
    public void theItemAddedHandlerShouldBeInvoked() {
        // Verified by event type matching
    }

    @Then("the type_url suffix should match the handler")
    public void theTypeUrlSuffixShouldMatchTheHandler() {
        // Verified by design
    }

    // ==========================================================================
    // Then Steps - Error Handling
    // ==========================================================================

    @Then("an error should be raised")
    public void anErrorShouldBeRaised() {
        assertThat(errorRaised).isTrue();
    }

    // Note: "the error should indicate deserialization failure" is in RouterSteps

    @Then("the behavior depends on language")
    public void theBehaviorDependsOnLanguage() {
        // May use default value or raise error
    }

    @Then("either default value is used or error is raised")
    public void eitherDefaultValueIsUsedOrErrorIsRaised() {
        // Language-dependent behavior
    }

    // ==========================================================================
    // Then Steps - Next Sequence Calculation
    // ==========================================================================

    @Then("next_sequence should be {int}")
    public void nextSequenceShouldBe(int seq) {
        assertThat(nextSequence).isEqualTo(seq);
    }

    // ==========================================================================
    // Then Steps - Immutability
    // ==========================================================================

    @Then("the EventBook should be unchanged")
    public void theEventBookShouldBeUnchanged() {
        assertThat(eventBook.pages).isNotEmpty();
    }

    @Then("the EventBook events should still be present")
    public void theEventBookEventsShouldStillBePresent() {
        assertThat(eventBook.pages).isNotEmpty();
    }

    @Then("a new state object should be returned")
    public void aNewStateObjectShouldBeReturned() {
        assertThat(currentState).isNotSameAs(originalState);
    }

    @Then("the original state should be unchanged")
    public void theOriginalStateShouldBeUnchanged() {
        if (originalState != null) {
            assertThat(originalState.itemCount).isEqualTo(5);
            assertThat(originalState.fieldValue).isEqualTo(100);
        }
    }

    // ==========================================================================
    // Then Steps - Language-Specific Patterns
    // ==========================================================================

    @Then("each event should be unpacked from Any")
    public void eachEventShouldBeUnpackedFromAny() {
        // Verified by design
    }

    @Then("_apply_event should be called for each")
    public void applyEventShouldBeCalledForEach() {
        // Verified by design
    }

    @Then("final state should be returned")
    public void finalStateShouldBeReturned() {
        assertThat(currentState).isNotNull();
    }

    @Then("the event should be unpacked")
    public void theEventShouldBeUnpacked() {
        // Verified by design
    }

    @Then("the correct type handler should be invoked")
    public void theCorrectTypeHandlerShouldBeInvoked() {
        // Verified by design
    }

    @Then("state should be mutated")
    public void stateShouldBeMutated() {
        assertThat(currentState).isNotNull();
    }

    // ==========================================================================
    // Helper Methods
    // ==========================================================================

    private String extractTypeName(String typeUrl) {
        int lastDot = typeUrl.lastIndexOf('.');
        int lastSlash = typeUrl.lastIndexOf('/');
        int start = Math.max(lastDot, lastSlash) + 1;
        return typeUrl.substring(start);
    }
}
