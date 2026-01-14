/**
 * Receipt projection state.
 */
export interface LineItem {
    productId: string;
    name: string;
    quantity: number;
    unitPriceCents: number;
}
export interface ReceiptState {
    customerId: string;
    items: LineItem[];
    subtotalCents: number;
    discountCents: number;
    finalTotalCents: number;
    paymentMethod: string;
    loyaltyPointsEarned: number;
    completedAt: {
        seconds: number;
        nanos: number;
    } | null;
    isCompleted: boolean;
}
export declare function emptyReceiptState(): ReceiptState;
