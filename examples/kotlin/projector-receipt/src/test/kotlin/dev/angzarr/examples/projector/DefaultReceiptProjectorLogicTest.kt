package dev.angzarr.examples.projector

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import examples.Domains.*
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertNull
import kotlin.test.assertTrue

class DefaultReceiptProjectorLogicTest {

    private lateinit var logic: ReceiptProjectorLogic

    @BeforeEach
    fun setUp() {
        logic = DefaultReceiptProjectorLogic()
    }

    // --- buildState tests ---

    @Test
    fun `buildState with empty event book returns empty state`() {
        val eventBook = EventBook.newBuilder().build()

        val state = logic.buildState(eventBook)

        assertFalse(state.isCompleted)
        assertEquals("", state.customerId)
        assertTrue(state.items.isEmpty())
    }

    @Test
    fun `buildState with TransactionCreated event`() {
        val item = LineItem.newBuilder()
            .setProductId("SKU-001")
            .setName("Widget")
            .setQuantity(2)
            .setUnitPriceCents(1000)
            .build()

        val event = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .addItems(item)
            .setSubtotalCents(2000)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val state = logic.buildState(eventBook)

        assertEquals("cust-123", state.customerId)
        assertEquals(1, state.items.size)
        assertEquals(2000, state.subtotalCents)
        assertFalse(state.isCompleted)
    }

    @Test
    fun `buildState with complete transaction flow`() {
        val item = LineItem.newBuilder()
            .setProductId("SKU-001")
            .setName("Widget")
            .setQuantity(2)
            .setUnitPriceCents(1000)
            .build()

        val created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .addItems(item)
            .setSubtotalCents(2000)
            .build()

        val discount = DiscountApplied.newBuilder()
            .setDiscountCents(200)
            .build()

        val completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(1800)
            .setPaymentMethod("card")
            .setLoyaltyPointsEarned(18)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(discount)).build())
            .addPages(EventPage.newBuilder().setNum(2).setEvent(Any.pack(completed)).build())
            .build()

        val state = logic.buildState(eventBook)

        assertTrue(state.isCompleted)
        assertEquals("cust-123", state.customerId)
        assertEquals(2000, state.subtotalCents)
        assertEquals(200, state.discountCents)
        assertEquals(1800, state.finalTotalCents)
        assertEquals("card", state.paymentMethod)
        assertEquals(18, state.loyaltyPointsEarned)
    }

    // --- formatReceipt tests ---

    @Test
    fun `formatReceipt contains items and totals`() {
        val item = LineItem.newBuilder()
            .setProductId("SKU-001")
            .setName("Widget")
            .setQuantity(2)
            .setUnitPriceCents(1000)
            .build()

        val state = ReceiptState(
            customerId = "cust-123",
            items = listOf(item),
            subtotalCents = 2000,
            discountCents = 200,
            finalTotalCents = 1800,
            paymentMethod = "card",
            loyaltyPointsEarned = 18,
            isCompleted = true
        )

        val receipt = logic.formatReceipt(state, "txn-001")

        assertTrue(receipt.contains("RECEIPT"))
        assertTrue(receipt.contains("Widget"))
        assertTrue(receipt.contains("20.0")) // subtotal
        assertTrue(receipt.contains("18.0")) // final total
        assertTrue(receipt.contains("card"))
        assertTrue(receipt.contains("18")) // loyalty points
    }

    // --- createProjection tests ---

    @Test
    fun `createProjection returns null for incomplete transaction`() {
        val event = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(2000)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val projection = logic.createProjection(eventBook, "receipt")

        assertNull(projection)
    }

    @Test
    fun `createProjection returns projection for completed transaction`() {
        val created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(2000)
            .build()

        val completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(2000)
            .setPaymentMethod("card")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(completed)).build())
            .build()

        val projection = logic.createProjection(eventBook, "receipt")

        assertNotNull(projection)
        assertEquals("receipt", projection.projector)
        assertTrue(projection.projection.typeUrl.endsWith("Receipt"))
    }
}
