package dev.angzarr.examples.projector

import examples.Domains.LineItem

/**
 * Immutable receipt projection state.
 */
data class ReceiptState(
    val customerId: String = "",
    val items: List<LineItem> = emptyList(),
    val subtotalCents: Int = 0,
    val discountCents: Int = 0,
    val finalTotalCents: Int = 0,
    val paymentMethod: String = "",
    val loyaltyPointsEarned: Int = 0,
    val completedAt: com.google.protobuf.Timestamp? = null,
    val isCompleted: Boolean = false
) {
    companion object {
        fun empty(): ReceiptState = ReceiptState()
    }
}
