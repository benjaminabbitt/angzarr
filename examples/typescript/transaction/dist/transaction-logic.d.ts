/**
 * Pure business logic for transaction aggregate.
 * No gRPC dependencies - can be tested in isolation.
 */
import { TransactionState, LineItem } from './transaction-state.js';
export interface TransactionCreatedEvent {
    customerId: string;
    items: LineItem[];
    subtotalCents: number;
    createdAt: {
        seconds: number;
        nanos: number;
    };
}
export interface DiscountAppliedEvent {
    discountType: string;
    value: number;
    discountCents: number;
    couponCode: string;
}
export interface TransactionCompletedEvent {
    finalTotalCents: number;
    paymentMethod: string;
    loyaltyPointsEarned: number;
    completedAt: {
        seconds: number;
        nanos: number;
    };
}
export interface TransactionCancelledEvent {
    reason: string;
    cancelledAt: {
        seconds: number;
        nanos: number;
    };
}
export type TransactionEvent = {
    type: 'TransactionCreated';
    data: TransactionCreatedEvent;
} | {
    type: 'DiscountApplied';
    data: DiscountAppliedEvent;
} | {
    type: 'TransactionCompleted';
    data: TransactionCompletedEvent;
} | {
    type: 'TransactionCancelled';
    data: TransactionCancelledEvent;
};
export interface EventPage {
    typeUrl: string;
    data: any;
}
export interface EventBook {
    pages: EventPage[];
    snapshot?: {
        state?: any;
    };
}
/**
 * Transaction business logic operations.
 */
export declare class TransactionLogic {
    /**
     * Rebuilds transaction state from an event history.
     */
    rebuildState(eventBook: EventBook | null | undefined): TransactionState;
    private applyEvent;
    /**
     * Calculates subtotal from line items.
     */
    calculateSubtotal(items: LineItem[]): number;
    /**
     * Handles a CreateTransaction command.
     */
    handleCreateTransaction(state: TransactionState, customerId: string, items: LineItem[]): TransactionCreatedEvent;
    /**
     * Handles an ApplyDiscount command.
     */
    handleApplyDiscount(state: TransactionState, discountType: string, value: number, couponCode?: string): DiscountAppliedEvent;
    /**
     * Handles a CompleteTransaction command.
     */
    handleCompleteTransaction(state: TransactionState, paymentMethod: string): TransactionCompletedEvent;
    /**
     * Handles a CancelTransaction command.
     */
    handleCancelTransaction(state: TransactionState, reason: string): TransactionCancelledEvent;
    private now;
}
