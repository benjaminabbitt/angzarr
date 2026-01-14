package dev.angzarr.examples.projector.cucumber

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import dev.angzarr.examples.projector.DefaultTransactionLogProjectorLogic
import dev.angzarr.examples.projector.LogEntry
import dev.angzarr.examples.projector.LogProjectorLogic
import examples.Domains.CustomerCreated
import examples.Domains.LoyaltyPointsAdded
import examples.Domains.TransactionCompleted
import examples.Domains.TransactionCreated
import io.cucumber.java.Before
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import kotlin.test.assertTrue

class LogProjectorStepDefinitions {

    private lateinit var logic: LogProjectorLogic
    private var priorEvents: MutableList<Any> = mutableListOf()
    private var logEntries: List<LogEntry> = emptyList()
    private var isTransactionDomainEvent: Boolean = false

    @Before
    fun setUp() {
        logic = DefaultTransactionLogProjectorLogic()
        priorEvents = mutableListOf()
        logEntries = emptyList()
        isTransactionDomainEvent = false
    }

    // --- Given steps ---

    @Given("a CustomerCreated event with name {string} and email {string}")
    fun customerCreatedEvent(name: String, email: String) {
        val event = CustomerCreated.newBuilder()
            .setName(name)
            .setEmail(email)
            .build()
        priorEvents.add(Any.pack(event))
        isTransactionDomainEvent = false
    }

    @Given("a LoyaltyPointsAdded event with {int} points and new_balance {int}")
    fun loyaltyPointsAddedEvent(points: Int, newBalance: Int) {
        val event = LoyaltyPointsAdded.newBuilder()
            .setPoints(points)
            .setNewBalance(newBalance)
            .build()
        priorEvents.add(Any.pack(event))
        isTransactionDomainEvent = false
    }

    @Given("a TransactionCreated event with customer {string} and subtotal {int}")
    fun transactionCreatedEvent(customerId: String, subtotal: Int) {
        val event = TransactionCreated.newBuilder()
            .setCustomerId(customerId)
            .setSubtotalCents(subtotal)
            .build()
        priorEvents.add(Any.pack(event))
        isTransactionDomainEvent = true
    }

    @Given("a TransactionCompleted event with total {int} and payment {string}")
    fun transactionCompletedEvent(total: Int, payment: String) {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(total)
            .setPaymentMethod(payment)
            .build()
        priorEvents.add(Any.pack(event))
        isTransactionDomainEvent = true
    }

    @Given("an unknown event type")
    fun unknownEventType() {
        val anyEvent = Any.newBuilder()
            .setTypeUrl("type.googleapis.com/unknown.Event")
            .build()
        priorEvents.add(anyEvent)
        isTransactionDomainEvent = false
    }

    // --- When steps ---

    @When("I process the log projector")
    fun processLogProjector() {
        val eventBook = buildEventBook()
        logEntries = logic.processEvents(eventBook)
    }

    // --- Then steps ---

    @Then("the event is logged successfully")
    fun eventIsLoggedSuccessfully() {
        assertTrue(logEntries.isNotEmpty(), "Expected at least one log entry")
        // Transaction log projector handles transaction events, customer events are unknown
        if (isTransactionDomainEvent) {
            assertTrue(logEntries[0].eventType != "unknown", "Transaction event should not be unknown type")
        } else {
            // Customer events are expected to be unknown to transaction projector
            assertTrue(logEntries[0].eventType == "unknown", "Customer event should be unknown to transaction projector")
        }
    }

    @Then("the event is logged as unknown")
    fun eventIsLoggedAsUnknown() {
        assertTrue(logEntries.isNotEmpty(), "Expected at least one log entry")
        assertTrue(logEntries[0].eventType == "unknown", "Event should be unknown type")
    }

    // --- Helpers ---

    private fun buildEventBook(): EventBook {
        val builder = EventBook.newBuilder()
        priorEvents.forEachIndexed { index, event ->
            builder.addPages(
                EventPage.newBuilder()
                    .setNum(index)
                    .setEvent(event)
                    .build()
            )
        }
        return builder.build()
    }
}
