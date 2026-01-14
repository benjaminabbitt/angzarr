/**
 * Immutable transaction aggregate state.
 */
export interface TransactionState {
  customerId: string;
  items: LineItem[];
  subtotalCents: number;
  discountCents: number;
  discountType: string;
  status: '' | 'pending' | 'completed' | 'cancelled';
}

export interface LineItem {
  productId: string;
  name: string;
  quantity: number;
  unitPriceCents: number;
}

export function emptyTransactionState(): TransactionState {
  return {
    customerId: '',
    items: [],
    subtotalCents: 0,
    discountCents: 0,
    discountType: '',
    status: '',
  };
}

export function transactionExists(state: TransactionState): boolean {
  return state.status !== '';
}

export function isPending(state: TransactionState): boolean {
  return state.status === 'pending';
}
