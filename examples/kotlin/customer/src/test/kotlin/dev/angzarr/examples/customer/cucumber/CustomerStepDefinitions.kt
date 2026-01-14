package dev.angzarr.examples.customer.cucumber

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import dev.angzarr.examples.customer.CommandValidationException
import dev.angzarr.examples.customer.CustomerLogic
import dev.angzarr.examples.customer.CustomerState
import dev.angzarr.examples.customer.DefaultCustomerLogic
import examples.Domains.CustomerCreated
import examples.Domains.LoyaltyPointsAdded
import examples.Domains.LoyaltyPointsRedeemed
import io.cucumber.java.Before
import io.cucumber.java.en.And
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import io.grpc.Status
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class CustomerStepDefinitions {

    private lateinit var logic: CustomerLogic
    private var priorEvents: MutableList<Any> = mutableListOf()
    private var resultEventBook: EventBook? = null
    private var error: CommandValidationException? = null
    private var state: CustomerState? = null
    private var pageNum = 0

    @Before
    fun setUp() {
        logic = DefaultCustomerLogic()
        priorEvents = mutableListOf()
        resultEventBook = null
        error = null
        state = null
        pageNum = 0
    }

    // --- Given steps ---

    @Given("no prior events for the aggregate")
    fun noPriorEventsForAggregate() {
        priorEvents.clear()
    }

    @Given("a CustomerCreated event with name {string} and email {string}")
    fun customerCreatedEvent(name: String, email: String) {
        val event = CustomerCreated.newBuilder()
            .setName(name)
            .setEmail(email)
            .build()
        priorEvents.add(Any.pack(event))
    }

    @Given("a LoyaltyPointsAdded event with {int} points and new_balance {int}")
    fun loyaltyPointsAddedEvent(points: Int, newBalance: Int) {
        val event = LoyaltyPointsAdded.newBuilder()
            .setPoints(points)
            .setNewBalance(newBalance)
            .build()
        priorEvents.add(Any.pack(event))
    }

    @Given("a LoyaltyPointsRedeemed event with {int} points and new_balance {int}")
    fun loyaltyPointsRedeemedEvent(points: Int, newBalance: Int) {
        val event = LoyaltyPointsRedeemed.newBuilder()
            .setPoints(points)
            .setNewBalance(newBalance)
            .build()
        priorEvents.add(Any.pack(event))
    }

    // --- When steps ---

    @When("I handle a CreateCustomer command with name {string} and email {string}")
    fun handleCreateCustomerCommand(name: String, email: String) {
        val eventBook = buildEventBook()
        state = logic.rebuildState(eventBook)
        try {
            resultEventBook = logic.handleCreateCustomer(state!!, name, email)
            error = null
        } catch (e: CommandValidationException) {
            error = e
            resultEventBook = null
        }
    }

    @When("I handle an AddLoyaltyPoints command with {int} points and reason {string}")
    fun handleAddLoyaltyPointsCommand(points: Int, reason: String) {
        val eventBook = buildEventBook()
        state = logic.rebuildState(eventBook)
        try {
            resultEventBook = logic.handleAddLoyaltyPoints(state!!, points, reason)
            error = null
        } catch (e: CommandValidationException) {
            error = e
            resultEventBook = null
        }
    }

    @When("I handle a RedeemLoyaltyPoints command with {int} points and type {string}")
    fun handleRedeemLoyaltyPointsCommand(points: Int, redemptionType: String) {
        val eventBook = buildEventBook()
        state = logic.rebuildState(eventBook)
        try {
            resultEventBook = logic.handleRedeemLoyaltyPoints(state!!, points, redemptionType)
            error = null
        } catch (e: CommandValidationException) {
            error = e
            resultEventBook = null
        }
    }

    @When("I rebuild the customer state")
    fun rebuildCustomerState() {
        val eventBook = buildEventBook()
        state = logic.rebuildState(eventBook)
    }

    // --- Then steps ---

    @Then("the result is a CustomerCreated event")
    fun resultIsCustomerCreatedEvent() {
        assertNotNull(resultEventBook, "Expected result but got error: ${error?.message}")
        assertTrue(resultEventBook!!.pagesCount > 0)
        assertTrue(
            resultEventBook!!.getPages(0).event.typeUrl.endsWith("CustomerCreated"),
            "Expected CustomerCreated event"
        )
    }

    @Then("the result is a LoyaltyPointsAdded event")
    fun resultIsLoyaltyPointsAddedEvent() {
        assertNotNull(resultEventBook, "Expected result but got error: ${error?.message}")
        assertTrue(resultEventBook!!.pagesCount > 0)
        assertTrue(
            resultEventBook!!.getPages(0).event.typeUrl.endsWith("LoyaltyPointsAdded"),
            "Expected LoyaltyPointsAdded event"
        )
    }

    @Then("the result is a LoyaltyPointsRedeemed event")
    fun resultIsLoyaltyPointsRedeemedEvent() {
        assertNotNull(resultEventBook, "Expected result but got error: ${error?.message}")
        assertTrue(resultEventBook!!.pagesCount > 0)
        assertTrue(
            resultEventBook!!.getPages(0).event.typeUrl.endsWith("LoyaltyPointsRedeemed"),
            "Expected LoyaltyPointsRedeemed event"
        )
    }

    @Then("the command fails with status {string}")
    fun commandFailsWithStatus(statusName: String) {
        assertNotNull(error, "Expected command to fail but it succeeded")
        val expectedCode = Status.Code.valueOf(statusName)
        assertEquals(expectedCode, error!!.statusCode, "Expected status $statusName")
    }

    @And("the error message contains {string}")
    fun errorMessageContains(substring: String) {
        assertNotNull(error, "Expected error but command succeeded")
        assertTrue(
            error!!.message?.contains(substring, ignoreCase = true) == true,
            "Expected error message to contain '$substring' but was '${error!!.message}'"
        )
    }

    @And("the event has name {string}")
    fun eventHasName(name: String) {
        val event = extractCustomerCreatedEvent()
        assertEquals(name, event.name)
    }

    @And("the event has email {string}")
    fun eventHasEmail(email: String) {
        val event = extractCustomerCreatedEvent()
        assertEquals(email, event.email)
    }

    @And("the event has points {int}")
    fun eventHasPoints(points: Int) {
        val eventAny = resultEventBook!!.getPages(0).event
        val actualPoints = when {
            eventAny.typeUrl.endsWith("LoyaltyPointsAdded") -> {
                eventAny.unpack(LoyaltyPointsAdded::class.java).points
            }
            eventAny.typeUrl.endsWith("LoyaltyPointsRedeemed") -> {
                eventAny.unpack(LoyaltyPointsRedeemed::class.java).points
            }
            else -> throw AssertionError("Event is not a points event: ${eventAny.typeUrl}")
        }
        assertEquals(points, actualPoints)
    }

    @And("the event has new_balance {int}")
    fun eventHasNewBalance(newBalance: Int) {
        val eventAny = resultEventBook!!.getPages(0).event
        val actualBalance = when {
            eventAny.typeUrl.endsWith("LoyaltyPointsAdded") -> {
                eventAny.unpack(LoyaltyPointsAdded::class.java).newBalance
            }
            eventAny.typeUrl.endsWith("LoyaltyPointsRedeemed") -> {
                eventAny.unpack(LoyaltyPointsRedeemed::class.java).newBalance
            }
            else -> throw AssertionError("Event is not a points event: ${eventAny.typeUrl}")
        }
        assertEquals(newBalance, actualBalance)
    }

    @And("the event has reason {string}")
    fun eventHasReason(reason: String) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("LoyaltyPointsAdded"))
        val event = eventAny.unpack(LoyaltyPointsAdded::class.java)
        assertEquals(reason, event.reason)
    }

    @And("the event has redemption_type {string}")
    fun eventHasRedemptionType(redemptionType: String) {
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("LoyaltyPointsRedeemed"))
        val event = eventAny.unpack(LoyaltyPointsRedeemed::class.java)
        assertEquals(redemptionType, event.redemptionType)
    }

    @Then("the state has name {string}")
    fun stateHasName(name: String) {
        assertNotNull(state)
        assertEquals(name, state!!.name)
    }

    @And("the state has email {string}")
    fun stateHasEmail(email: String) {
        assertNotNull(state)
        assertEquals(email, state!!.email)
    }

    @And("the state has loyalty_points {int}")
    fun stateHasLoyaltyPoints(points: Int) {
        assertNotNull(state)
        assertEquals(points, state!!.loyaltyPoints)
    }

    @And("the state has lifetime_points {int}")
    fun stateHasLifetimePoints(points: Int) {
        assertNotNull(state)
        assertEquals(points, state!!.lifetimePoints)
    }

    // --- Helpers ---

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

    private fun extractCustomerCreatedEvent(): CustomerCreated {
        assertNotNull(resultEventBook)
        val eventAny = resultEventBook!!.getPages(0).event
        assertTrue(eventAny.typeUrl.endsWith("CustomerCreated"))
        return eventAny.unpack(CustomerCreated::class.java)
    }
}
