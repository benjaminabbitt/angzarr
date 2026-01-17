package dev.angzarr.examples.fulfillment

data class FulfillmentState(
    val orderId: String = "",
    val status: String = "",
    val trackingNumber: String = ""
) {
    fun exists(): Boolean = orderId.isNotEmpty()
    fun isPending(): Boolean = status == "pending"
    fun isPicking(): Boolean = status == "picking"
    fun isPacking(): Boolean = status == "packing"
    fun isShipped(): Boolean = status == "shipped"
    fun isDelivered(): Boolean = status == "delivered"

    companion object {
        fun empty(): FulfillmentState = FulfillmentState()
    }
}
