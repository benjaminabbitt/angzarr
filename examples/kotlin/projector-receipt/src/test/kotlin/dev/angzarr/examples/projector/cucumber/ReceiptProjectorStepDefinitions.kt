package dev.angzarr.examples.projector.cucumber

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import dev.angzarr.Projection
import com.google.protobuf.Any
import dev.angzarr.examples.projector.DefaultReceiptProjectorLogic
import dev.angzarr.examples.projector.ReceiptProjectorLogic
import examples.Domains.DiscountApplied
import examples.Domains.LineItem
import examples.Domains.Receipt
import examples.Domains.TransactionCompleted
import examples.Domains.TransactionCreated
import io.cucumber.datatable.DataTable
import io.cucumber.java.Before
import io.cucumber.java.en.And
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class ReceiptProjectorStepDefinitions {

    private lateinit var logic: ReceiptProjectorLogic
    private var priorEvents: MutableList<Any> = mutableListOf()
    private var projection: Projection? = null
    private var receipt: Receipt? = null

    @Before
    fun setUp() {
        logic = DefaultReceiptProjectorLogic()
        priorEvents = mutableListOf()
        projection = null
        receipt = null
    }

    // --- Given steps ---

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

    @Given("a TransactionCompleted event with total {int} and payment {string}")
    fun transactionCompletedEvent(total: Int, payment: String) {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(total)
            .setPaymentMethod(payment)
            .build()
        priorEvents.add(Any.pack(event))
    }

    @Given("a TransactionCompleted event with total {int} and payment {string} earning {int} points")
    fun transactionCompletedEventWithPoints(total: Int, payment: String, points: Int) {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(total)
            .setPaymentMethod(payment)
            .setLoyaltyPointsEarned(points)
            .build()
        priorEvents.add(Any.pack(event))
    }

    // --- When steps ---

    @When("I project the events")
    fun projectTheEvents() {
        val eventBook = buildEventBook()
        projection = logic.createProjection(eventBook, "receipt")
        if (projection != null && projection!!.hasProjection()) {
            val projAny = projection!!.projection
            if (projAny.typeUrl.endsWith("Receipt")) {
                receipt = projAny.unpack(Receipt::class.java)
            }
        }
    }

    // --- Then steps ---

    @Then("no projection is generated")
    fun noProjectionIsGenerated() {
        assertNull(projection, "Expected no projection but got one")
    }

    @Then("a Receipt projection is generated")
    fun receiptProjectionIsGenerated() {
        assertNotNull(projection, "Expected a projection but got null")
        assertNotNull(receipt, "Expected a receipt but got null")
    }

    @And("the receipt has customer_id {string}")
    fun receiptHasCustomerId(customerId: String) {
        assertNotNull(receipt)
        assertEquals(customerId, receipt!!.customerId)
    }

    @And("the receipt has subtotal_cents {int}")
    fun receiptHasSubtotalCents(subtotalCents: Int) {
        assertNotNull(receipt)
        assertEquals(subtotalCents, receipt!!.subtotalCents)
    }

    @And("the receipt has discount_cents {int}")
    fun receiptHasDiscountCents(discountCents: Int) {
        assertNotNull(receipt)
        assertEquals(discountCents, receipt!!.discountCents)
    }

    @And("the receipt has final_total_cents {int}")
    fun receiptHasFinalTotalCents(finalTotalCents: Int) {
        assertNotNull(receipt)
        assertEquals(finalTotalCents, receipt!!.finalTotalCents)
    }

    @And("the receipt has payment_method {string}")
    fun receiptHasPaymentMethod(paymentMethod: String) {
        assertNotNull(receipt)
        assertEquals(paymentMethod, receipt!!.paymentMethod)
    }

    @And("the receipt has loyalty_points_earned {int}")
    fun receiptHasLoyaltyPointsEarned(points: Int) {
        assertNotNull(receipt)
        assertEquals(points, receipt!!.loyaltyPointsEarned)
    }

    @And("the receipt formatted_text contains {string}")
    fun receiptFormattedTextContains(substring: String) {
        assertNotNull(receipt)
        assertTrue(
            receipt!!.formattedText.contains(substring),
            "Expected formatted_text to contain '$substring' but was: ${receipt!!.formattedText}"
        )
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
