package dev.angzarr.examples.projector

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import examples.Domains.CustomerCreated
import examples.Domains.LoyaltyPointsAdded
import examples.Domains.LoyaltyPointsRedeemed
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class DefaultCustomerLogProjectorLogicTest {

    private lateinit var logic: LogProjectorLogic

    @BeforeEach
    fun setUp() {
        logic = DefaultCustomerLogProjectorLogic()
    }

    @Test
    fun `processEvents returns empty for empty event book`() {
        val eventBook = EventBook.newBuilder().build()

        val entries = logic.processEvents(eventBook)

        assertTrue(entries.isEmpty())
    }

    @Test
    fun `processEvents handles CustomerCreated event`() {
        val event = CustomerCreated.newBuilder()
            .setName("Alice")
            .setEmail("alice@example.com")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("CustomerCreated", entries[0].eventType)
        assertEquals("Alice", entries[0].fields["name"])
        assertEquals("alice@example.com", entries[0].fields["email"])
    }

    @Test
    fun `processEvents handles LoyaltyPointsAdded event`() {
        val event = LoyaltyPointsAdded.newBuilder()
            .setPoints(100)
            .setNewBalance(100)
            .setReason("welcome bonus")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("LoyaltyPointsAdded", entries[0].eventType)
        assertEquals("100", entries[0].fields["points"])
        assertEquals("100", entries[0].fields["balance"])
        assertEquals("welcome bonus", entries[0].fields["reason"])
    }

    @Test
    fun `processEvents handles LoyaltyPointsRedeemed event`() {
        val event = LoyaltyPointsRedeemed.newBuilder()
            .setPoints(50)
            .setNewBalance(50)
            .setRedemptionType("discount")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(1, entries.size)
        assertEquals("LoyaltyPointsRedeemed", entries[0].eventType)
        assertEquals("50", entries[0].fields["points"])
        assertEquals("50", entries[0].fields["balance"])
        assertEquals("discount", entries[0].fields["type"])
    }

    @Test
    fun `processEvents handles unknown event type`() {
        // Use an event that's not CustomerCreated, LoyaltyPointsAdded, or LoyaltyPointsRedeemed
        // We'll use a simple empty Any for this test
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
        val created = CustomerCreated.newBuilder()
            .setName("Bob")
            .setEmail("bob@example.com")
            .build()

        val added = LoyaltyPointsAdded.newBuilder()
            .setPoints(50)
            .setNewBalance(50)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(added)).build())
            .build()

        val entries = logic.processEvents(eventBook)

        assertEquals(2, entries.size)
        assertEquals("CustomerCreated", entries[0].eventType)
        assertEquals("LoyaltyPointsAdded", entries[1].eventType)
        assertEquals(0, entries[0].sequence)
        assertEquals(1, entries[1].sequence)
    }
}
