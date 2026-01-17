package dev.angzarr.examples.inventory

data class Reservation(
    val orderId: String,
    val quantity: Int
)

data class InventoryState(
    val productId: String = "",
    val onHand: Int = 0,
    val reserved: Int = 0,
    val reservations: List<Reservation> = emptyList(),
    val lowStockThreshold: Int = 10
) {
    fun exists(): Boolean = productId.isNotEmpty()
    fun available(): Int = onHand - reserved

    companion object {
        fun empty(): InventoryState = InventoryState()
    }
}
