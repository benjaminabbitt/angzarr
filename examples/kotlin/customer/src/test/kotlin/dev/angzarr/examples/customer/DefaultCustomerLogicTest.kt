package dev.angzarr.examples.customer

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import examples.Domains.CustomerCreated
import examples.Domains.LoyaltyPointsAdded
import examples.Domains.LoyaltyPointsRedeemed
import io.grpc.Status
import org.junit.jupiter.api.BeforeEach
import org.junit.jupiter.api.Test
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertFalse
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class DefaultCustomerLogicTest {

    private lateinit var logic: CustomerLogic

    @BeforeEach
    fun setUp() {
        logic = DefaultCustomerLogic()
    }

    // --- rebuildState tests ---

    @Test
    fun `rebuildState with null returns empty state`() {
        val state = logic.rebuildState(null)

        assertFalse(state.exists())
        assertEquals("", state.name)
        assertEquals("", state.email)
        assertEquals(0, state.loyaltyPoints)
        assertEquals(0, state.lifetimePoints)
    }

    @Test
    fun `rebuildState with empty event book returns empty state`() {
        val eventBook = EventBook.newBuilder().build()

        val state = logic.rebuildState(eventBook)

        assertFalse(state.exists())
    }

    @Test
    fun `rebuildState with CustomerCreated event`() {
        val event = CustomerCreated.newBuilder()
            .setName("John Doe")
            .setEmail("john@example.com")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(
                EventPage.newBuilder()
                    .setNum(0)
                    .setEvent(Any.pack(event))
                    .build()
            )
            .build()

        val state = logic.rebuildState(eventBook)

        assertTrue(state.exists())
        assertEquals("John Doe", state.name)
        assertEquals("john@example.com", state.email)
        assertEquals(0, state.loyaltyPoints)
    }

    @Test
    fun `rebuildState with LoyaltyPointsAdded event`() {
        val created = CustomerCreated.newBuilder()
            .setName("John Doe")
            .setEmail("john@example.com")
            .build()

        val added = LoyaltyPointsAdded.newBuilder()
            .setPoints(100)
            .setNewBalance(100)
            .setReason("welcome bonus")
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(added)).build())
            .build()

        val state = logic.rebuildState(eventBook)

        assertEquals(100, state.loyaltyPoints)
        assertEquals(100, state.lifetimePoints)
    }

    @Test
    fun `rebuildState with points added and redeemed`() {
        val created = CustomerCreated.newBuilder()
            .setName("John Doe")
            .setEmail("john@example.com")
            .build()

        val added = LoyaltyPointsAdded.newBuilder()
            .setPoints(100)
            .setNewBalance(100)
            .build()

        val redeemed = LoyaltyPointsRedeemed.newBuilder()
            .setPoints(30)
            .setNewBalance(70)
            .build()

        val eventBook = EventBook.newBuilder()
            .addPages(EventPage.newBuilder().setNum(0).setEvent(Any.pack(created)).build())
            .addPages(EventPage.newBuilder().setNum(1).setEvent(Any.pack(added)).build())
            .addPages(EventPage.newBuilder().setNum(2).setEvent(Any.pack(redeemed)).build())
            .build()

        val state = logic.rebuildState(eventBook)

        assertEquals(70, state.loyaltyPoints)
        assertEquals(100, state.lifetimePoints) // Lifetime not reduced
    }

    // --- handleCreateCustomer tests ---

    @Test
    fun `handleCreateCustomer success`() {
        val state = CustomerState.empty()

        val result = logic.handleCreateCustomer(state, "Jane Doe", "jane@example.com")

        assertNotNull(result)
        assertEquals(1, result.pagesCount)

        val eventAny = result.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("CustomerCreated"))
    }

    @Test
    fun `handleCreateCustomer already exists throws`() {
        val state = CustomerState(name = "Existing", email = "existing@test.com")

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCreateCustomer(state, "New Name", "new@test.com")
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
        assertTrue(ex.message!!.contains("already exists"))
    }

    @Test
    fun `handleCreateCustomer empty name throws`() {
        val state = CustomerState.empty()

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCreateCustomer(state, "", "email@test.com")
        }

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.statusCode)
        assertTrue(ex.message!!.contains("name"))
    }

    @Test
    fun `handleCreateCustomer empty email throws`() {
        val state = CustomerState.empty()

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleCreateCustomer(state, "Name", "")
        }

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.statusCode)
        assertTrue(ex.message!!.contains("email"))
    }

    // --- handleAddLoyaltyPoints tests ---

    @Test
    fun `handleAddLoyaltyPoints success`() {
        val state = CustomerState(name = "John", email = "john@test.com", loyaltyPoints = 50, lifetimePoints = 100)

        val result = logic.handleAddLoyaltyPoints(state, 25, "purchase")

        assertNotNull(result)
        assertEquals(1, result.pagesCount)
        assertTrue(result.getPages(0).event.typeUrl.endsWith("LoyaltyPointsAdded"))
    }

    @Test
    fun `handleAddLoyaltyPoints customer not exists throws`() {
        val state = CustomerState.empty()

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleAddLoyaltyPoints(state, 25, "purchase")
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
    }

    @Test
    fun `handleAddLoyaltyPoints zero points throws`() {
        val state = CustomerState(name = "John", email = "john@test.com", loyaltyPoints = 50)

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleAddLoyaltyPoints(state, 0, "purchase")
        }

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.statusCode)
    }

    @Test
    fun `handleAddLoyaltyPoints negative points throws`() {
        val state = CustomerState(name = "John", email = "john@test.com", loyaltyPoints = 50)

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleAddLoyaltyPoints(state, -10, "purchase")
        }

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.statusCode)
    }

    // --- handleRedeemLoyaltyPoints tests ---

    @Test
    fun `handleRedeemLoyaltyPoints success`() {
        val state = CustomerState(name = "John", email = "john@test.com", loyaltyPoints = 100, lifetimePoints = 200)

        val result = logic.handleRedeemLoyaltyPoints(state, 50, "discount")

        assertNotNull(result)
        assertEquals(1, result.pagesCount)
        assertTrue(result.getPages(0).event.typeUrl.endsWith("LoyaltyPointsRedeemed"))
    }

    @Test
    fun `handleRedeemLoyaltyPoints customer not exists throws`() {
        val state = CustomerState.empty()

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleRedeemLoyaltyPoints(state, 50, "discount")
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
    }

    @Test
    fun `handleRedeemLoyaltyPoints insufficient points throws`() {
        val state = CustomerState(name = "John", email = "john@test.com", loyaltyPoints = 30)

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleRedeemLoyaltyPoints(state, 50, "discount")
        }

        assertEquals(Status.Code.FAILED_PRECONDITION, ex.statusCode)
        assertTrue(ex.message!!.contains("Insufficient"))
    }

    @Test
    fun `handleRedeemLoyaltyPoints zero points throws`() {
        val state = CustomerState(name = "John", email = "john@test.com", loyaltyPoints = 100)

        val ex = assertFailsWith<CommandValidationException> {
            logic.handleRedeemLoyaltyPoints(state, 0, "discount")
        }

        assertEquals(Status.Code.INVALID_ARGUMENT, ex.statusCode)
    }
}
