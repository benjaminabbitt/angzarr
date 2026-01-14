package dev.angzarr.examples.customer

import dev.angzarr.EventBook

/**
 * Interface for customer business logic operations.
 * Enables IoC and testing without gRPC dependencies.
 */
interface CustomerLogic {

    /**
     * Rebuilds customer state from an event history.
     *
     * @param eventBook the event history (may be null for new aggregates)
     * @return the reconstructed customer state
     */
    fun rebuildState(eventBook: EventBook?): CustomerState

    /**
     * Handles a CreateCustomer command.
     *
     * @param state the current customer state
     * @param name the customer name
     * @param email the customer email
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    @Throws(CommandValidationException::class)
    fun handleCreateCustomer(state: CustomerState, name: String, email: String): EventBook

    /**
     * Handles an AddLoyaltyPoints command.
     *
     * @param state the current customer state
     * @param points the points to add
     * @param reason the reason for adding points
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    @Throws(CommandValidationException::class)
    fun handleAddLoyaltyPoints(state: CustomerState, points: Int, reason: String): EventBook

    /**
     * Handles a RedeemLoyaltyPoints command.
     *
     * @param state the current customer state
     * @param points the points to redeem
     * @param redemptionType the type of redemption
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    @Throws(CommandValidationException::class)
    fun handleRedeemLoyaltyPoints(state: CustomerState, points: Int, redemptionType: String): EventBook
}
