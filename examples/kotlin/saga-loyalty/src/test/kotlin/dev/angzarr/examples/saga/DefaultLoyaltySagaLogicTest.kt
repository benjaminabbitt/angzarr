package dev.angzarr.examples.saga

import dev.angzarr.Cover
import dev.angzarr.EventBook
import dev.angzarr.EventPage
import dev.angzarr.UUID
import com.google.protobuf.Any
import com.google.protobuf.ByteString
import examples.Domains.TransactionCompleted
import examples.Domains.TransactionCreated
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertTrue

class DefaultLoyaltySagaLogicTest {

    private lateinit var logic: LoyaltySagaLogic

    @BeforeEach
    fun setUp() {
        logic = DefaultLoyaltySagaLogic()
    }

    @Test
    fun `processEvents returns empty for empty event book`() {
        val eventBook = EventBook.newBuilder().build()

        val commands = logic.processEvents(eventBook)

        assertTrue(commands.isEmpty())
    }

    @Test
    fun `processEvents returns empty for non-completed transaction`() {
        val event = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(2000)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val commands = logic.processEvents(eventBook)

        assertTrue(commands.isEmpty())
    }

    @Test
    fun `processEvents generates command for completed transaction with points`() {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(2000)
            .setLoyaltyPointsEarned(20)
            .setPaymentMethod("card")
            .build()

        val rootId = UUID.newBuilder()
            .setValue(ByteString.copyFrom(ByteArray(16) { it.toByte() }))
            .build()

        val eventBook = EventBook.newBuilder()
            .setCover(Cover.newBuilder().setDomain("transaction").setRoot(rootId).build())
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val commands = logic.processEvents(eventBook)

        assertEquals(1, commands.size)
        assertEquals("customer", commands[0].cover.domain)
        assertTrue(commands[0].pagesList[0].command.typeUrl.endsWith("AddLoyaltyPoints"))
    }

    @Test
    fun `processEvents returns empty for completed transaction with zero points`() {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(50) // Very small transaction, 0 points
            .setLoyaltyPointsEarned(0)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(event)).build())
            .build()

        val commands = logic.processEvents(eventBook)

        assertTrue(commands.isEmpty())
    }
}
