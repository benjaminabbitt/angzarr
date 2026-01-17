package dev.angzarr.examples.cart

import examples.Domains.LineItem

data class CartState(
    val customerId: String = "",
    val items: List<LineItem> = emptyList(),
    val subtotalCents: Int = 0,
    val couponCode: String = "",
    val discountCents: Int = 0,
    val status: String = ""
) {
    fun exists(): Boolean = customerId.isNotEmpty()
    fun isActive(): Boolean = status == "active"
    fun isCheckedOut(): Boolean = status == "checked_out"

    companion object {
        fun empty(): CartState = CartState()
    }
}
