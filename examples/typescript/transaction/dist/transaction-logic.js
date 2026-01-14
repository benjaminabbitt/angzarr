/**
 * Pure business logic for transaction aggregate.
 * No gRPC dependencies - can be tested in isolation.
 */
import { CommandValidationError, } from './command-validation-error.js';
import { emptyTransactionState, transactionExists, isPending, } from './transaction-state.js';
/**
 * Transaction business logic operations.
 */
export class TransactionLogic {
    /**
     * Rebuilds transaction state from an event history.
     */
    rebuildState(eventBook) {
        let state = emptyTransactionState();
        if (!eventBook?.pages?.length) {
            return state;
        }
        // Start from snapshot if present
        if (eventBook.snapshot?.state) {
            const snapState = eventBook.snapshot.state;
            state = {
                customerId: snapState.customerId || '',
                items: snapState.items || [],
                subtotalCents: snapState.subtotalCents || 0,
                discountCents: snapState.discountCents || 0,
                discountType: snapState.discountType || '',
                status: snapState.status || '',
            };
        }
        // Apply events
        for (const page of eventBook.pages) {
            state = this.applyEvent(state, page);
        }
        return state;
    }
    applyEvent(state, page) {
        const typeUrl = page.typeUrl || '';
        if (typeUrl.endsWith('TransactionCreated')) {
            return {
                ...state,
                customerId: page.data.customerId,
                items: page.data.items || [],
                subtotalCents: page.data.subtotalCents,
                status: 'pending',
            };
        }
        else if (typeUrl.endsWith('DiscountApplied')) {
            return {
                ...state,
                discountCents: page.data.discountCents,
                discountType: page.data.discountType,
            };
        }
        else if (typeUrl.endsWith('TransactionCompleted')) {
            return {
                ...state,
                status: 'completed',
            };
        }
        else if (typeUrl.endsWith('TransactionCancelled')) {
            return {
                ...state,
                status: 'cancelled',
            };
        }
        return state;
    }
    /**
     * Calculates subtotal from line items.
     */
    calculateSubtotal(items) {
        return items.reduce((sum, item) => sum + (item.quantity || 1) * (item.unitPriceCents || 0), 0);
    }
    /**
     * Handles a CreateTransaction command.
     */
    handleCreateTransaction(state, customerId, items) {
        if (transactionExists(state)) {
            throw CommandValidationError.failedPrecondition('Transaction already exists');
        }
        if (!customerId) {
            throw CommandValidationError.invalidArgument('Customer ID is required');
        }
        if (!items?.length) {
            throw CommandValidationError.invalidArgument('At least one item is required');
        }
        const subtotalCents = this.calculateSubtotal(items);
        return {
            customerId,
            items,
            subtotalCents,
            createdAt: this.now(),
        };
    }
    /**
     * Handles an ApplyDiscount command.
     */
    handleApplyDiscount(state, discountType, value, couponCode = '') {
        if (!isPending(state)) {
            throw CommandValidationError.failedPrecondition('Transaction not in pending state');
        }
        let discountCents = 0;
        if (discountType === 'percentage') {
            discountCents = Math.floor((state.subtotalCents * value) / 100);
        }
        else if (discountType === 'fixed') {
            discountCents = value;
        }
        return {
            discountType,
            value,
            discountCents,
            couponCode,
        };
    }
    /**
     * Handles a CompleteTransaction command.
     */
    handleCompleteTransaction(state, paymentMethod) {
        if (!isPending(state)) {
            throw CommandValidationError.failedPrecondition('Transaction not in pending state');
        }
        const finalTotalCents = state.subtotalCents - state.discountCents;
        const loyaltyPointsEarned = Math.floor(finalTotalCents / 100); // 1 point per dollar
        return {
            finalTotalCents,
            paymentMethod,
            loyaltyPointsEarned,
            completedAt: this.now(),
        };
    }
    /**
     * Handles a CancelTransaction command.
     */
    handleCancelTransaction(state, reason) {
        if (!isPending(state)) {
            throw CommandValidationError.failedPrecondition('Transaction not in pending state');
        }
        return {
            reason,
            cancelledAt: this.now(),
        };
    }
    now() {
        return { seconds: Math.floor(Date.now() / 1000), nanos: 0 };
    }
}
//# sourceMappingURL=transaction-logic.js.map