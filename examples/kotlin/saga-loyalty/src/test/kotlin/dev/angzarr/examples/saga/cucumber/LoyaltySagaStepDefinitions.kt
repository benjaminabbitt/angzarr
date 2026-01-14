package dev.angzarr.examples.saga.cucumber

import dev.angzarr.CommandBook
import dev.angzarr.Cover
import dev.angzarr.EventBook
import dev.angzarr.EventPage
import dev.angzarr.UUID
import com.google.protobuf.Any
import com.google.protobuf.ByteString
import dev.angzarr.examples.saga.DefaultLoyaltySagaLogic
import dev.angzarr.examples.saga.LoyaltySagaLogic
import examples.Domains.AddLoyaltyPoints
import examples.Domains.TransactionCompleted
import examples.Domains.TransactionCreated
import io.cucumber.java.Before
import io.cucumber.java.en.And
import io.cucumber.java.en.Given
import io.cucumber.java.en.Then
import io.cucumber.java.en.When
import kotlin.test.assertEquals
import kotlin.test.assertNotNull
import kotlin.test.assertTrue

class LoyaltySagaStepDefinitions {

    private lateinit var logic: LoyaltySagaLogic
    private var priorEvents: MutableList<Any> = mutableListOf()
    private var commands: List<CommandBook> = emptyList()
    private var lastCommand: AddLoyaltyPoints? = null

    @Before
    fun setUp() {
        logic = DefaultLoyaltySagaLogic()
        priorEvents = mutableListOf()
        commands = emptyList()
        lastCommand = null
    }

    // --- Given steps ---

    @Given("a TransactionCreated event with customer {string} and subtotal {int}")
    fun transactionCreatedEvent(customerId: String, subtotal: Int) {
        val event = TransactionCreated.newBuilder()
            .setCustomerId(customerId)
            .setSubtotalCents(subtotal)
            .build()
        priorEvents.add(Any.pack(event))
    }

    @Given("a TransactionCompleted event with {int} loyalty points earned")
    fun transactionCompletedEventWithPoints(points: Int) {
        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(points * 100) // Approximate
            .setLoyaltyPointsEarned(points)
            .setPaymentMethod("card")
            .build()
        priorEvents.add(Any.pack(event))
    }

    // --- When steps ---

    @When("I process the saga")
    fun processTheSaga() {
        val rootId = UUID.newBuilder()
            .setValue(ByteString.copyFrom(ByteArray(16) { it.toByte() }))
            .build()

        val eventBook = EventBook.newBuilder()
            .setCover(Cover.newBuilder().setDomain("transaction").setRoot(rootId).build())
            .apply {
                priorEvents.forEachIndexed { index, event ->
                    addPages(
                        EventPage.newBuilder()
                            .setNum(index)
                            .setEvent(event)
                            .build()
                    )
                }
            }
            .build()

        commands = logic.processEvents(eventBook)

        if (commands.isNotEmpty() && commands[0].pagesCount > 0) {
            val cmdAny = commands[0].getPages(0).command
            if (cmdAny.typeUrl.endsWith("AddLoyaltyPoints")) {
                lastCommand = cmdAny.unpack(AddLoyaltyPoints::class.java)
            }
        }
    }

    // --- Then steps ---

    @Then("no commands are generated")
    fun noCommandsAreGenerated() {
        assertTrue(commands.isEmpty(), "Expected no commands but got ${commands.size}")
    }

    @Then("an AddLoyaltyPoints command is generated")
    fun addLoyaltyPointsCommandIsGenerated() {
        assertTrue(commands.isNotEmpty(), "Expected at least one command")
        val cmdAny = commands[0].getPages(0).command
        assertTrue(
            cmdAny.typeUrl.endsWith("AddLoyaltyPoints"),
            "Expected AddLoyaltyPoints command but got ${cmdAny.typeUrl}"
        )
    }

    @And("the command has points {int}")
    fun commandHasPoints(points: Int) {
        assertNotNull(lastCommand, "No AddLoyaltyPoints command was generated")
        assertEquals(points, lastCommand!!.points)
    }

    @And("the command has domain {string}")
    fun commandHasDomain(domain: String) {
        assertTrue(commands.isNotEmpty(), "Expected at least one command")
        assertEquals(domain, commands[0].cover.domain)
    }

    @And("the command reason contains {string}")
    fun commandReasonContains(substring: String) {
        assertNotNull(lastCommand, "No AddLoyaltyPoints command was generated")
        assertTrue(
            lastCommand!!.reason.contains(substring, ignoreCase = true),
            "Expected reason to contain '$substring' but was '${lastCommand!!.reason}'"
        )
    }
}
