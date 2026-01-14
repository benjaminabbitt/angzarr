package dev.angzarr.examples.transaction

import dev.angzarr.EventBook
import examples.Domains.LineItem

/**
 * Interface for transaction business logic operations.
 * Enables IoC and testing without gRPC dependencies.
 */
interface TransactionLogic {

    /**
     * Rebuilds transaction state from an event history.
     *
     * @param eventBook the event history (may be null for new aggregates)
     * @return the reconstructed transaction state
     */
    fun rebuildState(eventBook: EventBook?): TransactionState

    /**
     * Handles a CreateTransaction command.
     *
     * @param state the current transaction state
     * @param customerId the customer ID
     * @param items the line items
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    @Throws(CommandValidationException::class)
    fun handleCreateTransaction(state: TransactionState, customerId: String, items: List<LineItem>): EventBook

    /**
     * Handles an ApplyDiscount command.
     *
     * @param state the current transaction state
     * @param discountType the type of discount (percentage or fixed)
     * @param value the discount value
     * @param couponCode optional coupon code
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    @Throws(CommandValidationException::class)
    fun handleApplyDiscount(state: TransactionState, discountType: String, value: Int, couponCode: String): EventBook

    /**
     * Handles a CompleteTransaction command.
     *
     * @param state the current transaction state
     * @param paymentMethod the payment method used
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    @Throws(CommandValidationException::class)
    fun handleCompleteTransaction(state: TransactionState, paymentMethod: String): EventBook

    /**
     * Handles a CancelTransaction command.
     *
     * @param state the current transaction state
     * @param reason the cancellation reason
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    @Throws(CommandValidationException::class)
    fun handleCancelTransaction(state: TransactionState, reason: String): EventBook
}
