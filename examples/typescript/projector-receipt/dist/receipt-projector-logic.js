/**
 * Pure business logic for receipt projector.
 * No gRPC dependencies - can be tested in isolation.
 */
import { emptyReceiptState } from './receipt-state.js';
/**
 * Receipt projector business logic.
 */
export class ReceiptProjectorLogic {
    /**
     * Builds receipt state from event history.
     */
    buildReceiptState(eventBook) {
        let state = emptyReceiptState();
        if (!eventBook?.pages?.length) {
            return state;
        }
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
            };
        }
        else if (typeUrl.endsWith('DiscountApplied')) {
            return {
                ...state,
                discountCents: page.data.discountCents,
            };
        }
        else if (typeUrl.endsWith('TransactionCompleted')) {
            return {
                ...state,
                finalTotalCents: page.data.finalTotalCents,
                paymentMethod: page.data.paymentMethod,
                loyaltyPointsEarned: page.data.loyaltyPointsEarned,
                completedAt: page.data.completedAt,
                isCompleted: true,
            };
        }
        return state;
    }
    /**
     * Projects events into a Receipt, or null if transaction not completed.
     */
    project(eventBook) {
        const state = this.buildReceiptState(eventBook);
        if (!state.isCompleted) {
            return null;
        }
        const transactionId = this.uuidToHex(eventBook?.cover?.root);
        return {
            transactionId,
            customerId: state.customerId,
            items: state.items,
            subtotalCents: state.subtotalCents,
            discountCents: state.discountCents,
            finalTotalCents: state.finalTotalCents,
            paymentMethod: state.paymentMethod,
            loyaltyPointsEarned: state.loyaltyPointsEarned,
            completedAt: state.completedAt,
            formattedText: this.formatReceipt(state),
        };
    }
    /**
     * Formats a receipt as a text string.
     */
    formatReceipt(state) {
        const lines = [];
        const width = 40;
        lines.push('='.repeat(width));
        lines.push('         RECEIPT');
        lines.push('='.repeat(width));
        lines.push('');
        for (const item of state.items || []) {
            const qty = item.quantity || 1;
            const price = this.formatCents(item.unitPriceCents || 0);
            const total = this.formatCents((item.unitPriceCents || 0) * qty);
            lines.push(`${item.name}`);
            lines.push(`  ${qty} x ${price} = ${total}`);
        }
        lines.push('-'.repeat(width));
        lines.push(`Subtotal: ${this.formatCents(state.subtotalCents || 0)}`);
        if (state.discountCents > 0) {
            lines.push(`Discount: -${this.formatCents(state.discountCents)}`);
        }
        lines.push(`Total: ${this.formatCents(state.finalTotalCents || 0)}`);
        lines.push('');
        lines.push(`Payment: ${state.paymentMethod || 'N/A'}`);
        if (state.loyaltyPointsEarned > 0) {
            lines.push(`Loyalty Points Earned: ${state.loyaltyPointsEarned}`);
        }
        lines.push('='.repeat(width));
        lines.push('       Thank you!');
        lines.push('='.repeat(width));
        return lines.join('\n');
    }
    formatCents(cents) {
        return `$${(cents / 100).toFixed(2)}`;
    }
    uuidToHex(uuid) {
        if (!uuid?.value)
            return '';
        return Buffer.from(uuid.value).toString('hex');
    }
}
//# sourceMappingURL=receipt-projector-logic.js.map