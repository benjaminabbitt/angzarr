package dev.angzarr.examples.transaction

import dev.angzarr.EventBook
import dev.angzarr.EventPage
import com.google.protobuf.Any
import com.google.protobuf.Timestamp
import examples.Domains.*
import org.slf4j.LoggerFactory

/**
 * Default implementation of transaction business logic.
 */
class DefaultTransactionLogic : TransactionLogic {

    private val logger = LoggerFactory.getLogger(DefaultTransactionLogic::class.java)

    override fun rebuildState(eventBook: EventBook?): TransactionState {
        if (eventBook == null || eventBook.pagesList.isEmpty()) {
            return TransactionState.empty()
        }

        var state = TransactionState.empty()

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: TransactionState, event: Any): TransactionState {
        return when {
            event.`is`(TransactionCreated::class.java) -> {
                val e = event.unpack(TransactionCreated::class.java)
                state.copy(
                    customerId = e.customerId,
                    items = e.itemsList,
                    subtotalCents = e.subtotalCents,
                    status = TransactionState.Status.PENDING
                )
            }
            event.`is`(DiscountApplied::class.java) -> {
                val e = event.unpack(DiscountApplied::class.java)
                state.copy(discountCents = e.discountCents, discountType = e.discountType)
            }
            event.`is`(TransactionCompleted::class.java) -> {
                state.copy(status = TransactionState.Status.COMPLETED)
            }
            event.`is`(TransactionCancelled::class.java) -> {
                state.copy(status = TransactionState.Status.CANCELLED)
            }
            else -> state
        }
    }

    override fun handleCreateTransaction(
        state: TransactionState,
        customerId: String,
        items: List<LineItem>
    ): EventBook {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Transaction already exists")
        }

        if (customerId.isEmpty()) {
            throw CommandValidationException.invalidArgument("Customer ID is required")
        }
        if (items.isEmpty()) {
            throw CommandValidationException.invalidArgument("Items are required")
        }

        val subtotal = items.sumOf { it.quantity * it.unitPriceCents }

        logger.info("creating_transaction customer_id={} items={} subtotal={}", customerId, items.size, subtotal)

        val event = TransactionCreated.newBuilder()
            .setCustomerId(customerId)
            .addAllItems(items)
            .setSubtotalCents(subtotal)
            .setCreatedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    override fun handleApplyDiscount(
        state: TransactionState,
        discountType: String,
        value: Int,
        couponCode: String
    ): EventBook {
        if (!state.isPending()) {
            throw CommandValidationException.failedPrecondition("Transaction is not pending")
        }

        val discountCents = if (discountType == "percentage") {
            (state.subtotalCents * value) / 100
        } else {
            value
        }

        logger.info("applying_discount type={} value={} discount_cents={}", discountType, value, discountCents)

        val event = DiscountApplied.newBuilder()
            .setDiscountType(discountType)
            .setValue(value)
            .setDiscountCents(discountCents)
            .setCouponCode(couponCode)
            .build()

        return createEventBook(event)
    }

    override fun handleCompleteTransaction(state: TransactionState, paymentMethod: String): EventBook {
        if (!state.isPending()) {
            throw CommandValidationException.failedPrecondition("Transaction is not pending")
        }

        val finalTotal = state.finalTotal()
        val loyaltyPoints = finalTotal / 100

        logger.info("completing_transaction final_total={} loyalty_points={}", finalTotal, loyaltyPoints)

        val event = TransactionCompleted.newBuilder()
            .setFinalTotalCents(finalTotal)
            .setPaymentMethod(paymentMethod)
            .setLoyaltyPointsEarned(loyaltyPoints)
            .setCompletedAt(nowTimestamp())
            .build()

        return createEventBook(event)
    }

    override fun handleCancelTransaction(state: TransactionState, reason: String): EventBook {
        if (!state.isPending()) {
            throw CommandValidationException.failedPrecondition("Transaction is not pending")
        }

        logger.info("cancelling_transaction reason={}", reason)

        val event = TransactionCancelled.newBuilder()
            .setReason(reason)
            .setCancelledAt(nowTimestamp())
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
