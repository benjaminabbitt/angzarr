/**
 * Pure business logic for loyalty saga.
 * No gRPC dependencies - can be tested in isolation.
 */
/**
 * Loyalty saga business logic.
 */
export class LoyaltySagaLogic {
    /**
     * Processes events and generates commands.
     */
    process(eventBook) {
        const commands = [];
        if (!eventBook?.pages?.length) {
            return commands;
        }
        const transactionId = this.uuidToHex(eventBook.cover?.root);
        for (const page of eventBook.pages) {
            const typeUrl = page.typeUrl || '';
            if (typeUrl.endsWith('TransactionCompleted')) {
                const points = page.data.loyaltyPointsEarned || 0;
                if (points > 0) {
                    commands.push({
                        domain: 'customer',
                        customerId: this.uuidToHex(eventBook.cover?.root),
                        points,
                        reason: `transaction:${transactionId}`,
                    });
                }
            }
        }
        return commands;
    }
    uuidToHex(uuid) {
        if (!uuid?.value)
            return '';
        return Buffer.from(uuid.value).toString('hex');
    }
}
//# sourceMappingURL=loyalty-saga-logic.js.map