package dev.angzarr.examples.customer

/**
 * Immutable customer aggregate state.
 */
data class CustomerState(
    val name: String = "",
    val email: String = "",
    val loyaltyPoints: Int = 0,
    val lifetimePoints: Int = 0
) {
    fun exists(): Boolean = name.isNotEmpty()

    fun withLoyaltyPoints(points: Int): CustomerState = copy(loyaltyPoints = points)

    fun addLifetimePoints(points: Int): CustomerState = copy(lifetimePoints = lifetimePoints + points)

    companion object {
        fun empty(): CustomerState = CustomerState()
    }
}
