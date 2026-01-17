package dev.angzarr.examples.projector

import examples.Domains.LineItem

/**
 * Immutable receipt projection state for Order domain.
 */
data class ReceiptState(
    val customerId: String = "",
    val items: List<LineItem> = emptyList(),
    val subtotalCents: Int = 0,
    val discountCents: Int = 0,
    val loyaltyPointsUsed: Int = 0,
    val finalTotalCents: Int = 0,
    val paymentMethod: String = "",
    val completedAt: com.google.protobuf.Timestamp? = null,
    val isCompleted: Boolean = false
) {
    companion object {
        fun empty(): ReceiptState = ReceiptState()
        const val POINTS_PER_DOLLAR = 10
    }
}
