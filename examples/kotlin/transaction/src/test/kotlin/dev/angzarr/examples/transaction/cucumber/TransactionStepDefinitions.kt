package dev.angzarr.examples.transaction.cucumber

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import dev.angzarr.examples.transaction.CommandValidationException
import dev.angzarr.examples.transaction.DefaultTransactionLogic
import dev.angzarr.examples.transaction.TransactionLogic
import dev.angzarr.examples.transaction.TransactionState
import examples.Domains.DiscountApplied
import examples.Domains.LineItem
import examples.Domains.TransactionCancelled
import examples.Domains.TransactionCompleted
import examples.Domains.TransactionCreated
import io.cucumber.datatable.DataTable
import io.cucumber.java.Before
import io.cucumber.java.en.And
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.grpc.Status
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class TransactionStepDefinitions {

    private lateinit var logic: TransactionLogic
    private var priorEvents: MutableList<Any> = mutableListOf()
    private var resultEventBook: EventBook? = null
    private var error: CommandValidationException? = null
    private var state: TransactionState? = null

    @Before
    fun setUp() {
        logic = DefaultTransactionLogic()
        priorEvents = mutableListOf()
        resultEventBook = null
        error = null
        state = null
    }

    // --- Given steps ---

    @Given("no prior events for the aggregate")
    fun noPriorEventsForAggregate() {
        priorEvents.clear()
    }

    @Given("a TransactionCreated event with customer {string} and subtotal {int}")
    fun transactionCreatedEvent(customerId: String, subtotalCents: Int) {
        val event = TransactionCreated.newBuilder()
            .setCustomerId(customerId)
            .setSubtotalCents(subtotalCents)
            .build()
        priorEvents.add(Any.pack(event))
    }

    @Given("a TransactionCreated event with customer {string} and items:")
    fun transactionCreatedEventWithItems(customerId: String, dataTable: DataTable) {
        val items = dataTable.asMaps().map { row ->
            LineItem.newBuilder()
                .setProductId(row["product_id"] ?: "")
                .setName(row["name"] ?: "")
                .setQuantity(row["quantity"]?.toIntOrNull() ?: 0)
                .setUnitPriceCents(row["unit_price_cents"]?.toIntOrNull() ?: 0)
                .build()
        }
        val subtotal = items.sumOf { it.quantity * it.unitPriceCents }
        val event = TransactionCreated.newBuilder()
            .setCustomerId(customerId)
            .addAllItems(items)
            .setSubtotalCents(subtotal)
            .build()
        priorEvents.add(Any.pack(event))
    }

    @Given("a DiscountApplied event with {int} cents discount")
    fun discountAppliedEvent(discountCents: Int) {
        val event = DiscountApplied.newBuilder()
            .setDiscountCents(discountCents)
            .build()
        priorEvents.add(Any.pack(event))
    }

    @Given("a TransactionCompleted event")
    fun transactionCompletedEvent() {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(0)
            .build()
        priorEvents.add(Any.pack(event))
    }

    // --- When steps ---

    @When("I handle a CreateTransaction command with customer {string} and items:")
    fun handleCreateTransactionCommand(customerId: String, dataTable: DataTable) {
        val items = dataTable.asMaps().map { row ->
            LineItem.newBuilder()
                .setProductId(row["product_id"] ?: "")
                .setName(row["name"] ?: "")
                .setQuantity(row["quantity"]?.toIntOrNull() ?: 0)
                .setUnitPriceCents(row["unit_price_cents"]?.toIntOrNull() ?: 0)
                .build()
        }
        executeCommand { logic.handleCreateTransaction(it, customerId, items) }
    }

    @When("I handle a CreateTransaction command with customer {string} and no items")
    fun handleCreateTransactionCommandNoItems(customerId: String) {
        executeCommand { logic.handleCreateTransaction(it, customerId, emptyList()) }
    }

    @When("I handle an ApplyDiscount command with type {string} and value {int}")
    fun handleApplyDiscountCommand(discountType: String, value: Int) {
        executeCommand { logic.handleApplyDiscount(it, discountType, value, "") }
    }

    @When("I handle a CompleteTransaction command with payment method {string}")
    fun handleCompleteTransactionCommand(paymentMethod: String) {
        executeCommand { logic.handleCompleteTransaction(it, paymentMethod) }
    }

    @When("I handle a CancelTransaction command with reason {string}")
    fun handleCancelTransactionCommand(reason: String) {
        executeCommand { logic.handleCancelTransaction(it, reason) }
    }

    @When("I rebuild the transaction state")
    fun rebuildTransactionState() {
        val eventBook = buildEventBook()
        state = logic.rebuildState(eventBook)
    }

    // --- Then steps ---

    @Then("the result is a TransactionCreated event")
    fun resultIsTransactionCreatedEvent() {
        assertNotNull(resultEventBook, "Expected result but got error: ${error?.message}")
        assertTrue(resultEventBook!!.pagesCount > 0)
        assertTrue(
            resultEventBook!!.getPages(0).event.typeUrl.endsWith("TransactionCreated"),
            "Expected TransactionCreated event"
        )
    }

    @Then("the result is a DiscountApplied event")
    fun resultIsDiscountAppliedEvent() {
        assertNotNull(resultEventBook, "Expected result but got error: ${error?.message}")
        assertTrue(resultEventBook!!.pagesCount > 0)
        assertTrue(
            resultEventBook!!.getPages(0).event.typeUrl.endsWith("DiscountApplied"),
            "Expected DiscountApplied event"
        )
    }

    @Then("the result is a TransactionCompleted event")
    fun resultIsTransactionCompletedEvent() {
        assertNotNull(resultEventBook, "Expected result but got error: ${error?.message}")
        assertTrue(resultEventBook!!.pagesCount > 0)
        assertTrue(
            resultEventBook!!.getPages(0).event.typeUrl.endsWith("TransactionCompleted"),
            "Expected TransactionCompleted event"
        )
    }

    @Then("the result is a TransactionCancelled event")
    fun resultIsTransactionCancelledEvent() {
        assertNotNull(resultEventBook, "Expected result but got error: ${error?.message}")
        assertTrue(resultEventBook!!.pagesCount > 0)
        assertTrue(
            resultEventBook!!.getPages(0).event.typeUrl.endsWith("TransactionCancelled"),
            "Expected TransactionCancelled event"
        )
    }

    @Then("the command fails with status {string}")
    fun commandFailsWithStatus(statusName: String) {
        assertNotNull(error, "Expected command to fail but it succeeded")
        val expectedCode = Status.Code.valueOf(statusName)
        assertEquals(expectedCode, error!!.statusCode, "Expected status $statusName")
    }

    @And("the event has customer_id {string}")
    fun eventHasCustomerId(customerId: String) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("TransactionCreated"))
        val event = eventAny.unpack(TransactionCreated::class.java)
        assertEquals(customerId, event.customerId)
    }

    @And("the event has subtotal_cents {int}")
    fun eventHasSubtotalCents(subtotalCents: Int) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("TransactionCreated"))
        val event = eventAny.unpack(TransactionCreated::class.java)
        assertEquals(subtotalCents, event.subtotalCents)
    }

    @And("the event has discount_cents {int}")
    fun eventHasDiscountCents(discountCents: Int) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("DiscountApplied"))
        val event = eventAny.unpack(DiscountApplied::class.java)
        assertEquals(discountCents, event.discountCents)
    }

    @And("the event has final_total_cents {int}")
    fun eventHasFinalTotalCents(finalTotalCents: Int) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("TransactionCompleted"))
        val event = eventAny.unpack(TransactionCompleted::class.java)
        assertEquals(finalTotalCents, event.finalTotalCents)
    }

    @And("the event has payment_method {string}")
    fun eventHasPaymentMethod(paymentMethod: String) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("TransactionCompleted"))
        val event = eventAny.unpack(TransactionCompleted::class.java)
        assertEquals(paymentMethod, event.paymentMethod)
    }

    @And("the event has loyalty_points_earned {int}")
    fun eventHasLoyaltyPointsEarned(points: Int) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("TransactionCompleted"))
        val event = eventAny.unpack(TransactionCompleted::class.java)
        assertEquals(points, event.loyaltyPointsEarned)
    }

    @And("the event has reason {string}")
    fun eventHasReason(reason: String) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("TransactionCancelled"))
        val event = eventAny.unpack(TransactionCancelled::class.java)
        assertEquals(reason, event.reason)
    }

    @Then("the state has customer_id {string}")
    fun stateHasCustomerId(customerId: String) {
        assertNotNull(state)
        assertEquals(customerId, state!!.customerId)
    }

    @And("the state has subtotal_cents {int}")
    fun stateHasSubtotalCents(subtotalCents: Int) {
        assertNotNull(state)
        assertEquals(subtotalCents, state!!.subtotalCents)
    }

    @And("the state has status {string}")
    fun stateHasStatus(statusName: String) {
        assertNotNull(state)
        val expectedStatus = when (statusName) {
            "pending" -> TransactionState.Status.PENDING
            "completed" -> TransactionState.Status.COMPLETED
            "cancelled" -> TransactionState.Status.CANCELLED
            else -> TransactionState.Status.NONE
        }
        assertEquals(expectedStatus, state!!.status)
    }

    // --- Helpers ---

    private fun executeCommand(command: (TransactionState) -> EventBook) {
        val eventBook = buildEventBook()
        state = logic.rebuildState(eventBook)
        try {
            resultEventBook = command(state!!)
            error = null
        } catch (e: CommandValidationException) {
            error = e
            resultEventBook = null
        }
    }

    private fun buildEventBook(): EventBook? {
        if (priorEvents.isEmpty()) {
            return null
        }
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
