package dev.angzarr.examples.customer

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import com.google.protobuf.Timestamp
import examples.Domains.CustomerCreated
import examples.Domains.LoyaltyPointsAdded
import examples.Domains.LoyaltyPointsRedeemed
import org.slf4j.LoggerFactory

/**
 * Default implementation of customer business logic.
 */
class DefaultCustomerLogic : CustomerLogic {

    private val logger = LoggerFactory.getLogger(DefaultCustomerLogic::class.java)

    override fun rebuildState(eventBook: EventBook?): CustomerState {
        if (eventBook == null || eventBook.pagesList.isEmpty()) {
            return CustomerState.empty()
        }

        var state = CustomerState.empty()

        // Start from snapshot if present
        eventBook.snapshot?.state?.let { snapAny ->
            if (snapAny.`is`(examples.Domains.CustomerState::class.java)) {
                val snapState = snapAny.unpack(examples.Domains.CustomerState::class.java)
                state = CustomerState(
                    name = snapState.name,
                    email = snapState.email,
                    loyaltyPoints = snapState.loyaltyPoints,
                    lifetimePoints = snapState.lifetimePoints
                )
            }
        }

        // Apply events
        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: CustomerState, event: Any): CustomerState {
        return when {
            event.`is`(CustomerCreated::class.java) -> {
                val e = event.unpack(CustomerCreated::class.java)
                state.copy(name = e.name, email = e.email)
            }
            event.`is`(LoyaltyPointsAdded::class.java) -> {
                val e = event.unpack(LoyaltyPointsAdded::class.java)
                state.copy(
                    loyaltyPoints = e.newBalance,
                    lifetimePoints = state.lifetimePoints + e.points
                )
            }
            event.`is`(LoyaltyPointsRedeemed::class.java) -> {
                val e = event.unpack(LoyaltyPointsRedeemed::class.java)
                state.copy(loyaltyPoints = e.newBalance)
            }
            else -> state
        }
    }

    override fun handleCreateCustomer(state: CustomerState, name: String, email: String): EventBook {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Customer already exists")
        }

        if (name.isEmpty()) {
            throw CommandValidationException.invalidArgument("Customer name is required")
        }
        if (email.isEmpty()) {
            throw CommandValidationException.invalidArgument("Customer email is required")
        }

        logger.info("creating_customer name={} email={}", name, email)

        val event = CustomerCreated.newBuilder()
            .setName(name)
            .setEmail(email)
            .setCreatedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    override fun handleAddLoyaltyPoints(state: CustomerState, points: Int, reason: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Customer does not exist")
        }

        if (points <= 0) {
            throw CommandValidationException.invalidArgument("Points must be positive")
        }

        val newBalance = state.loyaltyPoints + points

        logger.info("adding_loyalty_points points={} new_balance={} reason={}", points, newBalance, reason)

        val event = LoyaltyPointsAdded.newBuilder()
            .setPoints(points)
            .setNewBalance(newBalance)
            .setReason(reason)
            .build()

        return createEventBook(event)
    }

    override fun handleRedeemLoyaltyPoints(state: CustomerState, points: Int, redemptionType: String): EventBook {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Customer does not exist")
        }

        if (points <= 0) {
            throw CommandValidationException.invalidArgument("Points must be positive")
        }
        if (points > state.loyaltyPoints) {
            throw CommandValidationException.failedPrecondition(
                "Insufficient points: have ${state.loyaltyPoints}, need $points"
            )
        }

        val newBalance = state.loyaltyPoints - points

        logger.info("redeeming_loyalty_points points={} new_balance={} redemption_type={}", points, newBalance, redemptionType)

        val event = LoyaltyPointsRedeemed.newBuilder()
            .setPoints(points)
            .setNewBalance(newBalance)
            .setRedemptionType(redemptionType)
            .build()

        return createEventBook(event)
    }

    private fun createEventBook(event: com.google.protobuf.Message): EventBook {
        val page = EventPage.newBuilder()
            .setNum(0)
            .setEvent(Any.pack(event))
            .setCreatedAt(nowTimestamp())
            .build()

        return EventBook.newBuilder()
            .addPages(page)
            .build()
    }

    private fun nowTimestamp(): Timestamp = Timestamp.newBuilder()
        .setSeconds(System.currentTimeMillis() / 1000)
        .build()
}
