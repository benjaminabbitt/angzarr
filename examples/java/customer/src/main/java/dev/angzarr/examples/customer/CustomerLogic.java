package dev.angzarr.examples.customer;

import dev.angzarr.EventBook;

/**
 * Interface for customer business logic operations.
 * Enables IoC and testing without gRPC dependencies.
 */
public interface CustomerLogic {

    /**
     * Rebuilds customer state from an event history.
     *
     * @param eventBook the event history (may be null for new aggregates)
     * @return the reconstructed customer state
     */
    CustomerState rebuildState(EventBook eventBook);

    /**
     * Handles a CreateCustomer command.
     *
     * @param state the current customer state
     * @param name the customer name
     * @param email the customer email
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    EventBook handleCreateCustomer(CustomerState state, String name, String email)
        throws CommandValidationException;

    /**
     * Handles an AddLoyaltyPoints command.
     *
     * @param state the current customer state
     * @param points the points to add
     * @param reason the reason for adding points
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    EventBook handleAddLoyaltyPoints(CustomerState state, int points, String reason)
        throws CommandValidationException;

    /**
     * Handles a RedeemLoyaltyPoints command.
     *
     * @param state the current customer state
     * @param points the points to redeem
     * @param redemptionType the type of redemption
     * @return the resulting event book
     * @throws CommandValidationException if validation fails
     */
    EventBook handleRedeemLoyaltyPoints(CustomerState state, int points, String redemptionType)
        throws CommandValidationException;
}
