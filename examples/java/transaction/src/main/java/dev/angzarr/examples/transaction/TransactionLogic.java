package dev.angzarr.examples.transaction;

import examples.Domains.LineItem;
import dev.angzarr.EventBook;

import java.util.List;

/**
 * Interface for transaction business logic operations.
 */
public interface TransactionLogic {

    TransactionState rebuildState(EventBook eventBook);

    EventBook handleCreateTransaction(TransactionState state, String customerId, List<LineItem> items)
        throws CommandValidationException;

    EventBook handleApplyDiscount(TransactionState state, String discountType, int value, String couponCode)
        throws CommandValidationException;

    EventBook handleCompleteTransaction(TransactionState state, String paymentMethod)
        throws CommandValidationException;

    EventBook handleCancelTransaction(TransactionState state, String reason)
        throws CommandValidationException;
}
