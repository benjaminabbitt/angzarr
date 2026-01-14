/**
 * Pure business logic for transaction aggregate.
 * No gRPC dependencies - can be tested in isolation.
 */

import {
  CommandValidationError,
  StatusCode,
} from './command-validation-error.js';
import {
  TransactionState,
  LineItem,
  emptyTransactionState,
  transactionExists,
  isPending,
} from './transaction-state.js';

export interface TransactionCreatedEvent {
  customerId: string;
  items: LineItem[];
  subtotalCents: number;
  createdAt: { seconds: number; nanos: number };
}

export interface DiscountAppliedEvent {
  discountType: string;
  value: number;
  discountCents: number;
  couponCode: string;
}

export interface TransactionCompletedEvent {
  finalTotalCents: number;
  paymentMethod: string;
  loyaltyPointsEarned: number;
  completedAt: { seconds: number; nanos: number };
}

export interface TransactionCancelledEvent {
  reason: string;
  cancelledAt: { seconds: number; nanos: number };
}

export type TransactionEvent =
  | { type: 'TransactionCreated'; data: TransactionCreatedEvent }
  | { type: 'DiscountApplied'; data: DiscountAppliedEvent }
  | { type: 'TransactionCompleted'; data: TransactionCompletedEvent }
  | { type: 'TransactionCancelled'; data: TransactionCancelledEvent };

export interface EventPage {
  typeUrl: string;
  data: any;
}

export interface EventBook {
  pages: EventPage[];
  snapshot?: {
    state?: any;
  };
}

/**
 * Transaction business logic operations.
 */
export class TransactionLogic {
  /**
   * Rebuilds transaction state from an event history.
   */
  rebuildState(eventBook: EventBook | null | undefined): TransactionState {
    let state = emptyTransactionState();

    if (!eventBook?.pages?.length) {
      return state;
    }

    // Start from snapshot if present
    if (eventBook.snapshot?.state) {
      const snapState = eventBook.snapshot.state;
      state = {
        customerId: snapState.customerId || '',
        items: snapState.items || [],
        subtotalCents: snapState.subtotalCents || 0,
        discountCents: snapState.discountCents || 0,
        discountType: snapState.discountType || '',
        status: snapState.status || '',
      };
    }

    // Apply events
    for (const page of eventBook.pages) {
      state = this.applyEvent(state, page);
    }

    return state;
  }

  private applyEvent(state: TransactionState, page: EventPage): TransactionState {
    const typeUrl = page.typeUrl || '';

    if (typeUrl.endsWith('TransactionCreated')) {
      return {
        ...state,
        customerId: page.data.customerId,
        items: page.data.items || [],
        subtotalCents: page.data.subtotalCents,
        status: 'pending',
      };
    } else if (typeUrl.endsWith('DiscountApplied')) {
      return {
        ...state,
        discountCents: page.data.discountCents,
        discountType: page.data.discountType,
      };
    } else if (typeUrl.endsWith('TransactionCompleted')) {
      return {
        ...state,
        status: 'completed',
      };
    } else if (typeUrl.endsWith('TransactionCancelled')) {
      return {
        ...state,
        status: 'cancelled',
      };
    }

    return state;
  }

  /**
   * Calculates subtotal from line items.
   */
  calculateSubtotal(items: LineItem[]): number {
    return items.reduce(
      (sum, item) => sum + (item.quantity || 1) * (item.unitPriceCents || 0),
      0
    );
  }

  /**
   * Handles a CreateTransaction command.
   */
  handleCreateTransaction(
    state: TransactionState,
    customerId: string,
    items: LineItem[]
  ): TransactionCreatedEvent {
    if (transactionExists(state)) {
      throw CommandValidationError.failedPrecondition('Transaction already exists');
    }

    if (!customerId) {
      throw CommandValidationError.invalidArgument('Customer ID is required');
    }
    if (!items?.length) {
      throw CommandValidationError.invalidArgument('At least one item is required');
    }

    const subtotalCents = this.calculateSubtotal(items);

    return {
      customerId,
      items,
      subtotalCents,
      createdAt: this.now(),
    };
  }

  /**
   * Handles an ApplyDiscount command.
   */
  handleApplyDiscount(
    state: TransactionState,
    discountType: string,
    value: number,
    couponCode: string = ''
  ): DiscountAppliedEvent {
    if (!isPending(state)) {
      throw CommandValidationError.failedPrecondition('Transaction not in pending state');
    }

    let discountCents = 0;
    if (discountType === 'percentage') {
      discountCents = Math.floor((state.subtotalCents * value) / 100);
    } else if (discountType === 'fixed') {
      discountCents = value;
    }

    return {
      discountType,
      value,
      discountCents,
      couponCode,
    };
  }

  /**
   * Handles a CompleteTransaction command.
   */
  handleCompleteTransaction(
    state: TransactionState,
    paymentMethod: string
  ): TransactionCompletedEvent {
    if (!isPending(state)) {
      throw CommandValidationError.failedPrecondition('Transaction not in pending state');
    }

    const finalTotalCents = state.subtotalCents - state.discountCents;
    const loyaltyPointsEarned = Math.floor(finalTotalCents / 100); // 1 point per dollar

    return {
      finalTotalCents,
      paymentMethod,
      loyaltyPointsEarned,
      completedAt: this.now(),
    };
  }

  /**
   * Handles a CancelTransaction command.
   */
  handleCancelTransaction(
    state: TransactionState,
    reason: string
  ): TransactionCancelledEvent {
    if (!isPending(state)) {
      throw CommandValidationError.failedPrecondition('Transaction not in pending state');
    }

    return {
      reason,
      cancelledAt: this.now(),
    };
  }

  private now(): { seconds: number; nanos: number } {
    return { seconds: Math.floor(Date.now() / 1000), nanos: 0 };
  }
}
