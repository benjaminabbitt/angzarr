package dev.angzarr.examples.product

data class ProductState(
    val sku: String = "",
    val name: String = "",
    val description: String = "",
    val priceCents: Int = 0,
    val status: String = ""
) {
    fun exists(): Boolean = sku.isNotEmpty()
    fun isActive(): Boolean = status == "active"
    fun isDiscontinued(): Boolean = status == "discontinued"

    companion object {
        fun empty(): ProductState = ProductState()
    }
}
