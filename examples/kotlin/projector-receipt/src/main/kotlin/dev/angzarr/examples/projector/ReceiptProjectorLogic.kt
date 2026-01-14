package dev.angzarr.examples.projector

import dev.angzarr.EventBook
import dev.angzarr.Projection
import com.google.protobuf.Any
import examples.Domains.*

/**
 * Interface for receipt projector business logic.
 */
interface ReceiptProjectorLogic {
    fun buildState(eventBook: EventBook): ReceiptState
    fun formatReceipt(state: ReceiptState, transactionId: String): String
    fun createProjection(eventBook: EventBook, projectorName: String): Projection?
}

/**
 * Default implementation of receipt projector logic.
 */
class DefaultReceiptProjectorLogic : ReceiptProjectorLogic {

    override fun buildState(eventBook: EventBook): ReceiptState {
        var state = ReceiptState.empty()

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            state = applyEvent(state, event)
        }

        return state
    }

    private fun applyEvent(state: ReceiptState, event: Any): ReceiptState {
        return when {
            event.`is`(TransactionCreated::class.java) -> {
                val e = event.unpack(TransactionCreated::class.java)
                state.copy(
                    customerId = e.customerId,
                    items = e.itemsList,
                    subtotalCents = e.subtotalCents
                )
            }
            event.`is`(DiscountApplied::class.java) -> {
                val e = event.unpack(DiscountApplied::class.java)
                state.copy(discountCents = e.discountCents)
            }
            event.`is`(TransactionCompleted::class.java) -> {
                val e = event.unpack(TransactionCompleted::class.java)
                state.copy(
                    finalTotalCents = e.finalTotalCents,
                    paymentMethod = e.paymentMethod,
                    loyaltyPointsEarned = e.loyaltyPointsEarned,
                    completedAt = e.completedAt,
                    isCompleted = true
                )
            }
            else -> state
        }
    }

    override fun formatReceipt(state: ReceiptState, transactionId: String): String {
        val sb = StringBuilder()
        sb.appendLine("=".repeat(40))
        sb.appendLine("         RECEIPT")
        sb.appendLine("=".repeat(40))
        sb.appendLine()

        for (item in state.items) {
            val total = item.quantity * item.unitPriceCents
            sb.appendLine(item.name)
            sb.appendLine("  ${item.quantity} x $${item.unitPriceCents / 100.0} = $${total / 100.0}")
        }

        sb.appendLine("-".repeat(40))
        sb.appendLine("Subtotal: $${state.subtotalCents / 100.0}")
        if (state.discountCents > 0) {
            sb.appendLine("Discount: -$${state.discountCents / 100.0}")
        }
        sb.appendLine("Total: $${state.finalTotalCents / 100.0}")
        sb.appendLine()
        sb.appendLine("Payment: ${state.paymentMethod}")
        if (state.loyaltyPointsEarned > 0) {
            sb.appendLine("Loyalty Points Earned: ${state.loyaltyPointsEarned}")
        }
        sb.appendLine("=".repeat(40))
        sb.appendLine("       Thank you!")
        sb.appendLine("=".repeat(40))

        return sb.toString()
    }

    override fun createProjection(eventBook: EventBook, projectorName: String): Projection? {
        val state = buildState(eventBook)
        if (!state.isCompleted) return null

        val transactionId = eventBook.cover?.root?.value?.toByteArray()
            ?.joinToString("") { "%02x".format(it) } ?: ""

        val receipt = Receipt.newBuilder()
            .setTransactionId(transactionId)
            .setCustomerId(state.customerId)
            .addAllItems(state.items)
            .setSubtotalCents(state.subtotalCents)
            .setDiscountCents(state.discountCents)
            .setFinalTotalCents(state.finalTotalCents)
            .setPaymentMethod(state.paymentMethod)
            .setLoyaltyPointsEarned(state.loyaltyPointsEarned)
            .setCompletedAt(state.completedAt)
            .setFormattedText(formatReceipt(state, transactionId))
            .build()

        return Projection.newBuilder()
            .setCover(eventBook.cover)
            .setProjector(projectorName)
            .setSequence(eventBook.pagesList.size)
            .setProjection(Any.pack(receipt))
            .build()
    }
}
