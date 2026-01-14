export function emptyTransactionState() {
    return {
        customerId: '',
        items: [],
        subtotalCents: 0,
        discountCents: 0,
        discountType: '',
        status: '',
    };
}
export function transactionExists(state) {
    return state.status !== '';
}
export function isPending(state) {
    return state.status === 'pending';
}
//# sourceMappingURL=transaction-state.js.map