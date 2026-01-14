package dev.angzarr.examples.projector

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import examples.Domains.DiscountApplied
import examples.Domains.TransactionCancelled
import examples.Domains.TransactionCompleted
import examples.Domains.TransactionCreated
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class DefaultTransactionLogProjectorLogicTest {

    private lateinit var logic: LogProjectorLogic

    @BeforeEach
    fun setUp() {
        logic = DefaultTransactionLogProjectorLogic()
    }

    @Test
    fun `processEvents returns empty for empty event book`() {
        val eventBook = EventBook.newBuilder().build()

        val entries = logic.processEvents(eventBook)

        assertTrue(entries.isEmpty())
    }

    @Test
    fun `processEvents handles TransactionCreated event`() {
        val event = TransactionCreated.newBuilder()
            .setCustomerId("cust-001")
            .setSubtotalCents(2000)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("TransactionCreated", entries[0].eventType)
        assertEquals("cust-001", entries[0].fields["customer"])
        assertEquals("2000", entries[0].fields["subtotal"])
    }

    @Test
    fun `processEvents handles DiscountApplied event`() {
        val event = DiscountApplied.newBuilder()
            .setDiscountType("percentage")
            .setValue(10)
            .setDiscountCents(200)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("DiscountApplied", entries[0].eventType)
        assertEquals("percentage", entries[0].fields["discount_type"])
        assertEquals("10", entries[0].fields["value"])
        assertEquals("200", entries[0].fields["cents"])
    }

    @Test
    fun `processEvents handles TransactionCompleted event`() {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(2000)
            .setPaymentMethod("card")
            .setLoyaltyPointsEarned(20)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("TransactionCompleted", entries[0].eventType)
        assertEquals("2000", entries[0].fields["total"])
        assertEquals("card", entries[0].fields["payment"])
        assertEquals("20", entries[0].fields["points"])
    }

    @Test
    fun `processEvents handles TransactionCancelled event`() {
        val event = TransactionCancelled.newBuilder()
            .setReason("customer request")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("TransactionCancelled", entries[0].eventType)
        assertEquals("customer request", entries[0].fields["reason"])
    }

    @Test
    fun `processEvents handles unknown event type`() {
        val anyEvent = Any.newBuilder()
            .setTypeUrl("type.googleapis.com/unknown.Event")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(anyEvent).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("unknown", entries[0].eventType)
    }

    @Test
    fun `processEvents handles multiple events`() {
        val created = TransactionCreated.newBuilder()
            .setCustomerId("cust-002")
            .setSubtotalCents(3000)
            .build()

        val completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(3000)
            .setPaymentMethod("cash")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(completed)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(2, entries.size)
        assertEquals("TransactionCreated", entries[0].eventType)
        assertEquals("TransactionCompleted", entries[1].eventType)
        assertEquals(0, entries[0].sequence)
        assertEquals(1, entries[1].sequence)
    }
}
