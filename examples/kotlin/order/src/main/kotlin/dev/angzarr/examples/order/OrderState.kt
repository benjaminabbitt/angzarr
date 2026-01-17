package dev.angzarr.examples.order

import examples.Domains.LineItem

data class OrderState(
    val customerId: String = "",
    val items: List<LineItem> = emptyList(),
    val subtotalCents: Int = 0,
    val discountCents: Int = 0,
    val loyaltyPointsUsed: Int = 0,
    val finalTotalCents: Int = 0,
    val paymentMethod: String = "",
    val status: String = ""
) {
    fun exists(): Boolean = customerId.isNotEmpty()
    fun isPendingPayment(): Boolean = status == "pending_payment"
    fun isPaid(): Boolean = status == "paid"
    fun isCompleted(): Boolean = status == "completed"
    fun isCancelled(): Boolean = status == "cancelled"

    companion object {
        fun empty(): OrderState = OrderState()
    }
}
