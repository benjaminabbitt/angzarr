/**
 * Immutable customer aggregate state.
 */
export interface CustomerState {
    name: string;
    email: string;
    loyaltyPoints: number;
    lifetimePoints: number;
}
export declare function emptyCustomerState(): CustomerState;
export declare function customerExists(state: CustomerState): boolean;
