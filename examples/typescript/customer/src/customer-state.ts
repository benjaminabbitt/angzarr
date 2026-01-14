/**
 * Immutable customer aggregate state.
 */
export interface CustomerState {
  name: string;
  email: string;
  loyaltyPoints: number;
  lifetimePoints: number;
}

export function emptyCustomerState(): CustomerState {
  return {
    name: '',
    email: '',
    loyaltyPoints: 0,
    lifetimePoints: 0,
  };
}

export function customerExists(state: CustomerState): boolean {
  return state.name !== '';
}
