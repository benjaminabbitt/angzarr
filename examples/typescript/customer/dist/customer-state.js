export function emptyCustomerState() {
    return {
        name: '',
        email: '',
        loyaltyPoints: 0,
        lifetimePoints: 0,
    };
}
export function customerExists(state) {
    return state.name !== '';
}
//# sourceMappingURL=customer-state.js.map