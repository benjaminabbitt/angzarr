/**
 * Pure business logic for customer log projector.
 * No gRPC dependencies - can be tested in isolation.
 */
/**
 * Customer log projector business logic.
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
        const domain = eventBook.cover?.domain || 'customer';
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
            if (typeUrl.endsWith('CustomerCreated')) {
                entry.fields = {
                    name: page.data.name,
                    email: page.data.email,
                };
            }
            else if (typeUrl.endsWith('LoyaltyPointsAdded')) {
                entry.fields = {
                    points: page.data.points,
                    newBalance: page.data.newBalance,
                    reason: page.data.reason,
                };
            }
            else if (typeUrl.endsWith('LoyaltyPointsRedeemed')) {
                entry.fields = {
                    points: page.data.points,
                    newBalance: page.data.newBalance,
                    redemptionType: page.data.redemptionType,
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