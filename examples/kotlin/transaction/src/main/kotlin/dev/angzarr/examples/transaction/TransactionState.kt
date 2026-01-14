package dev.angzarr.examples.transaction

import examples.Domains.LineItem

/**
 * Immutable transaction aggregate state.
 */
data class TransactionState(
    val customerId: String = "",
    val items: List<LineItem> = emptyList(),
    val subtotalCents: Int = 0,
    val discountCents: Int = 0,
    val discountType: String = "",
    val status: Status = Status.NONE
) {
    enum class Status {
        NONE,
        PENDING,
        COMPLETED,
        CANCELLED
    }

    fun exists(): Boolean = status != Status.NONE

    fun isPending(): Boolean = status == Status.PENDING

    fun finalTotal(): Int = subtotalCents - discountCents

    companion object {
        fun empty(): TransactionState = TransactionState()
    }
}
