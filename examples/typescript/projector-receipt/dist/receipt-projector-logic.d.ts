/**
 * Pure business logic for receipt projector.
 * No gRPC dependencies - can be tested in isolation.
 */
import { ReceiptState, LineItem } from './receipt-state.js';
export interface EventPage {
    typeUrl: string;
    data: any;
}
export interface EventBook {
    cover?: {
        domain?: string;
        root?: {
            value?: Uint8Array;
        };
    };
    pages: EventPage[];
}
export interface Receipt {
    transactionId: string;
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
    formattedText: string;
}
/**
 * Receipt projector business logic.
 */
export declare class ReceiptProjectorLogic {
    /**
     * Builds receipt state from event history.
     */
    buildReceiptState(eventBook: EventBook | null | undefined): ReceiptState;
    private applyEvent;
    /**
     * Projects events into a Receipt, or null if transaction not completed.
     */
    project(eventBook: EventBook | null | undefined): Receipt | null;
    /**
     * Formats a receipt as a text string.
     */
    formatReceipt(state: ReceiptState): string;
    private formatCents;
    private uuidToHex;
}
