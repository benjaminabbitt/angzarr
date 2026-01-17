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
    fun formatReceipt(state: ReceiptState, orderId: String): String
    fun createProjection(eventBook: EventBook, projectorName: String): Projection?
}

/**
 * Default implementation of receipt projector logic for Order domain.
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
            event.`is`(OrderCreated::class.java) -> {
                val e = event.unpack(OrderCreated::class.java)
                state.copy(
                    customerId = e.customerId,
                    items = e.itemsList,
                    subtotalCents = e.subtotalCents,
                    discountCents = e.discountCents
                )
            }
            event.`is`(LoyaltyDiscountApplied::class.java) -> {
                val e = event.unpack(LoyaltyDiscountApplied::class.java)
                state.copy(
                    loyaltyPointsUsed = e.pointsUsed,
                    discountCents = state.discountCents + e.discountCents
                )
            }
            event.`is`(PaymentSubmitted::class.java) -> {
                val e = event.unpack(PaymentSubmitted::class.java)
                state.copy(
                    paymentMethod = e.paymentMethod,
                    finalTotalCents = e.amountCents
                )
            }
            event.`is`(OrderCompleted::class.java) -> {
                val e = event.unpack(OrderCompleted::class.java)
                state.copy(
                    completedAt = e.completedAt,
                    isCompleted = true
                )
            }
            else -> state
        }
    }

    override fun formatReceipt(state: ReceiptState, orderId: String): String {
        val sb = StringBuilder()
        val line = "═".repeat(40)
        val thinLine = "─".repeat(40)

        val shortOrderId = if (orderId.length > 16) orderId.take(16) else orderId
        val shortCustId = if (state.customerId.length > 16) state.customerId.take(16) else state.customerId

        sb.appendLine(line)
        sb.appendLine("           RECEIPT")
        sb.appendLine(line)
        sb.appendLine("Order: $shortOrderId...")
        sb.appendLine("Customer: ${if (shortCustId.isEmpty()) "N/A" else "$shortCustId..."}")
        sb.appendLine(thinLine)

        for (item in state.items) {
            val lineTotal = item.quantity * item.unitPriceCents
            sb.appendLine("${item.quantity} x ${item.name} @ $${String.format("%.2f", item.unitPriceCents / 100.0)} = $${String.format("%.2f", lineTotal / 100.0)}")
        }

        sb.appendLine(thinLine)
        sb.appendLine("Subtotal:              $${String.format("%.2f", state.subtotalCents / 100.0)}")

        if (state.discountCents > 0) {
            val discountType = if (state.loyaltyPointsUsed > 0) "loyalty" else "coupon"
            sb.appendLine("Discount ($discountType):       -$${String.format("%.2f", state.discountCents / 100.0)}")
        }

        sb.appendLine(thinLine)
        sb.appendLine("TOTAL:                 $${String.format("%.2f", state.finalTotalCents / 100.0)}")
        sb.appendLine("Payment: ${state.paymentMethod}")
        sb.appendLine(thinLine)

        val loyaltyPointsEarned = (state.finalTotalCents / 100) * ReceiptState.POINTS_PER_DOLLAR
        sb.appendLine("Loyalty Points Earned: $loyaltyPointsEarned")
        sb.appendLine(line)
        sb.appendLine("     Thank you for your purchase!")
        sb.append(line)

        return sb.toString()
    }

    override fun createProjection(eventBook: EventBook, projectorName: String): Projection? {
        val state = buildState(eventBook)
        if (!state.isCompleted) return null

        val orderId = eventBook.cover?.root?.value?.toByteArray()
            ?.joinToString("") { "%02x".format(it) } ?: ""

        val loyaltyPointsEarned = (state.finalTotalCents / 100) * ReceiptState.POINTS_PER_DOLLAR

        val receipt = Receipt.newBuilder()
            .setOrderId(orderId)
            .setCustomerId(state.customerId)
            .addAllItems(state.items)
            .setSubtotalCents(state.subtotalCents)
            .setDiscountCents(state.discountCents)
            .setFinalTotalCents(state.finalTotalCents)
            .setPaymentMethod(state.paymentMethod)
            .setLoyaltyPointsEarned(loyaltyPointsEarned)
            .setFormattedText(formatReceipt(state, orderId))
            .build()

        return Projection.newBuilder()
            .setCover(eventBook.cover)
            .setProjector(projectorName)
            .setSequence(eventBook.pagesList.size)
            .setProjection(Any.pack(receipt))
            .build()
    }
}
