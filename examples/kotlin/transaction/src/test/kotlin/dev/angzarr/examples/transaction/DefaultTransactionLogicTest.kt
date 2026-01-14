package dev.angzarr.examples.transaction

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import examples.Domains.*
import io.grpc.Status
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class DefaultTransactionLogicTest {

    private lateinit var logic: TransactionLogic

    @BeforeEach
    fun setUp() {
        logic = DefaultTransactionLogic()
    }

    // --- rebuildState tests ---

    @Test
    fun `rebuildState with null returns empty state`() {
        val state = logic.rebuildState(null)

        assertFalse(state.exists())
        assertEquals("", state.customerId)
        assertTrue(state.items.isEmpty())
        assertEquals(0, state.subtotalCents)
    }

    @Test
    fun `rebuildState with TransactionCreated event`() {
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

        val state = logic.rebuildState(eventBook)

        assertTrue(state.exists())
        assertTrue(state.isPending())
        assertEquals("cust-123", state.customerId)
        assertEquals(1, state.items.size)
        assertEquals(2000, state.subtotalCents)
    }

    @Test
    fun `rebuildState with discount applied`() {
        val created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(2000)
            .build()

        val discount = DiscountApplied.newBuilder()
            .setDiscountType("percentage")
            .setValue(10)
            .setDiscountCents(200)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(discount)).build())
            .build()

        val state = logic.rebuildState(eventBook)

        assertEquals(200, state.discountCents)
        assertEquals("percentage", state.discountType)
        assertEquals(1800, state.finalTotal())
    }

    @Test
    fun `rebuildState with completed transaction`() {
        val created = TransactionCreated.newBuilder()
            .setCustomerId("cust-123")
            .setSubtotalCents(2000)
            .build()

        val completed = TransactionCompleted.newBuilder()
            .setFinalTotalCents(2000)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(completed)).build())
            .build()

        val state = logic.rebuildState(eventBook)

        assertEquals(TransactionState.Status.COMPLETED, state.status)
        assertFalse(state.isPending())
    }

    // --- handleCreateTransaction tests ---

    @Test
    fun `handleCreateTransaction success`() {
        val state = TransactionState.empty()
        val items = listOf(
            LineItem.newBuilder()
                .setProductId("SKU-001")
                .setName("Widget")
                .setQuantity(2)
                .setUnitPriceCents(1000)
                .build()
        )

        val result = logic.handleCreateTransaction(state, "cust-123", items)

        assertNotNull(result)
        assertEquals(1, result.pagesCount)
        assertTrue(result.getPages(0).event.typeUrl.endsWith("TransactionCreated"))
    }

    @Test
    fun `handleCreateTransaction already exists throws`() {
        val state = TransactionState(status = TransactionState.Status.PENDING, customerId = "cust-123")
        val items = listOf(LineItem.newBuilder().setProductId("SKU-001").build())

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCreateTransaction(state, "cust-456", items)
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
    }

    @Test
    fun `handleCreateTransaction empty customerId throws`() {
        val state = TransactionState.empty()
        val items = listOf(LineItem.newBuilder().setProductId("SKU-001").build())

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCreateTransaction(state, "", items)
        }

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.statusCode)
    }

    @Test
    fun `handleCreateTransaction empty items throws`() {
        val state = TransactionState.empty()

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCreateTransaction(state, "cust-123", emptyList())
        }

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.statusCode)
    }

    // --- handleApplyDiscount tests ---

    @Test
    fun `handleApplyDiscount success with percentage`() {
        val state = TransactionState(
            status = TransactionState.Status.PENDING,
            customerId = "cust-123",
            subtotalCents = 2000
        )

        val result = logic.handleApplyDiscount(state, "percentage", 10, "SAVE10")

        assertNotNull(result)
        assertEquals(1, result.pagesCount)
        assertTrue(result.getPages(0).event.typeUrl.endsWith("DiscountApplied"))
    }

    @Test
    fun `handleApplyDiscount not pending throws`() {
        val state = TransactionState(status = TransactionState.Status.COMPLETED)

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleApplyDiscount(state, "percentage", 10, "")
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
    }

    // --- handleCompleteTransaction tests ---

    @Test
    fun `handleCompleteTransaction success`() {
        val state = TransactionState(
            status = TransactionState.Status.PENDING,
            customerId = "cust-123",
            subtotalCents = 2000
        )

        val result = logic.handleCompleteTransaction(state, "card")

        assertNotNull(result)
        assertEquals(1, result.pagesCount)
        assertTrue(result.getPages(0).event.typeUrl.endsWith("TransactionCompleted"))
    }

    @Test
    fun `handleCompleteTransaction not pending throws`() {
        val state = TransactionState.empty()

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCompleteTransaction(state, "card")
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
    }

    // --- handleCancelTransaction tests ---

    @Test
    fun `handleCancelTransaction success`() {
        val state = TransactionState(
            status = TransactionState.Status.PENDING,
            customerId = "cust-123"
        )

        val result = logic.handleCancelTransaction(state, "customer request")

        assertNotNull(result)
        assertEquals(1, result.pagesCount)
        assertTrue(result.getPages(0).event.typeUrl.endsWith("TransactionCancelled"))
    }

    @Test
    fun `handleCancelTransaction not pending throws`() {
        val state = TransactionState(status = TransactionState.Status.COMPLETED)

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCancelTransaction(state, "too late")
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
    }
}
