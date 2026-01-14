/**
 * Pure business logic for transaction log projector.
 * No gRPC dependencies - can be tested in isolation.
 */
/**
 * Transaction log projector business logic.
 */
export class LogProjectorLogic {
    /**
     * Processes an event book and returns log entries.
     */
    processEvents(eventBook) {
        const entries = [];
        if (!eventBook?.pages?.length) {
            return entries;
        }
        const domain = eventBook.cover?.domain || 'transaction';
        const rootId = this.uuidToHex(eventBook.cover?.root);
        const shortId = rootId.slice(0, 16);
        for (let i = 0; i < eventBook.pages.length; i++) {
            const page = eventBook.pages[i];
            const typeUrl = page.typeUrl || '';
            const eventType = typeUrl.split('.').pop() || typeUrl;
            const sequence = i;
            const entry = {
                domain,
                rootId: shortId,
                sequence,
                eventType,
                fields: {},
            };
            if (typeUrl.endsWith('TransactionCreated')) {
                entry.fields = {
                    customerId: page.data.customerId,
                    itemCount: page.data.items?.length || 0,
                    subtotalCents: page.data.subtotalCents,
                };
            }
            else if (typeUrl.endsWith('DiscountApplied')) {
                entry.fields = {
                    discountType: page.data.discountType,
                    value: page.data.value,
                    discountCents: page.data.discountCents,
                };
            }
            else if (typeUrl.endsWith('TransactionCompleted')) {
                entry.fields = {
                    finalTotalCents: page.data.finalTotalCents,
                    paymentMethod: page.data.paymentMethod,
                    loyaltyPointsEarned: page.data.loyaltyPointsEarned,
                };
            }
            else if (typeUrl.endsWith('TransactionCancelled')) {
                entry.fields = {
                    reason: page.data.reason,
                };
            }
            else {
                entry.fields = { unknown: true };
            }
            entries.push(entry);
        }
        return entries;
    }
    uuidToHex(uuid) {
        if (!uuid?.value)
            return '';
        return Buffer.from(uuid.value).toString('hex');
    }
}
//# sourceMappingURL=log-projector-logic.js.map