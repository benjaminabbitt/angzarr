/**
 * Immutable transaction aggregate state.
 */
export interface TransactionState {
    customerId: string;
    items: LineItem[];
    subtotalCents: number;
    discountCents: number;
    discountType: string;
    status: '' | 'pending' | 'completed' | 'cancelled';
}
export interface LineItem {
    productId: string;
    name: string;
    quantity: number;
    unitPriceCents: number;
}
export declare function emptyTransactionState(): TransactionState;
export declare function transactionExists(state: TransactionState): boolean;
export declare function isPending(state: TransactionState): boolean;
