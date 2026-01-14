import { describe, it, expect, beforeEach } from 'vitest';
import { TransactionLogic, EventBook } from '../transaction-logic.js';
import { TransactionState, emptyTransactionState, LineItem } from '../transaction-state.js';
import { CommandValidationError, StatusCode } from '../command-validation-error.js';

describe('TransactionLogic', () => {
  let logic: TransactionLogic;

  beforeEach(() => {
    logic = new TransactionLogic();
  });

  describe('rebuildState', () => {
    it('returns empty state for null event book', () => {
      const state = logic.rebuildState(null);

      expect(state.customerId).toBe('');
      expect(state.items).toEqual([]);
      expect(state.subtotalCents).toBe(0);
      expect(state.status).toBe('');
    });

    it('returns empty state for empty event book', () => {
      const eventBook: EventBook = { pages: [] };

      const state = logic.rebuildState(eventBook);

      expect(state.customerId).toBe('');
      expect(state.status).toBe('');
    });

    it('applies TransactionCreated event', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCreated',
            data: {
              customerId: 'cust-001',
              items: [{ productId: 'SKU-001', name: 'Widget', quantity: 2, unitPriceCents: 1000 }],
              subtotalCents: 2000,
            },
          },
        ],
      };

      const state = logic.rebuildState(eventBook);

      expect(state.customerId).toBe('cust-001');
      expect(state.subtotalCents).toBe(2000);
      expect(state.status).toBe('pending');
    });

    it('applies DiscountApplied event', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCreated',
            data: { customerId: 'cust-001', items: [], subtotalCents: 2000 },
          },
          {
            typeUrl: 'type.examples/examples.DiscountApplied',
            data: { discountType: 'percentage', discountCents: 200 },
          },
        ],
      };

      const state = logic.rebuildState(eventBook);

      expect(state.discountCents).toBe(200);
      expect(state.discountType).toBe('percentage');
    });

    it('applies TransactionCompleted event', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCreated',
            data: { customerId: 'cust-001', items: [], subtotalCents: 2000 },
          },
          {
            typeUrl: 'type.examples/examples.TransactionCompleted',
            data: { finalTotalCents: 2000 },
          },
        ],
      };

      const state = logic.rebuildState(eventBook);

      expect(state.status).toBe('completed');
    });

    it('applies TransactionCancelled event', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCreated',
            data: { customerId: 'cust-001', items: [], subtotalCents: 2000 },
          },
          {
            typeUrl: 'type.examples/examples.TransactionCancelled',
            data: { reason: 'customer request' },
          },
        ],
      };

      const state = logic.rebuildState(eventBook);

      expect(state.status).toBe('cancelled');
    });
  });

  describe('calculateSubtotal', () => {
    it('calculates subtotal from items', () => {
      const items: LineItem[] = [
        { productId: 'SKU-001', name: 'Widget', quantity: 2, unitPriceCents: 1000 },
        { productId: 'SKU-002', name: 'Gadget', quantity: 1, unitPriceCents: 2500 },
      ];

      const subtotal = logic.calculateSubtotal(items);

      expect(subtotal).toBe(4500);
    });

    it('returns 0 for empty items', () => {
      expect(logic.calculateSubtotal([])).toBe(0);
    });
  });

  describe('handleCreateTransaction', () => {
    it('returns TransactionCreated event for new transaction', () => {
      const state = emptyTransactionState();
      const items: LineItem[] = [
        { productId: 'SKU-001', name: 'Widget', quantity: 2, unitPriceCents: 1000 },
      ];

      const event = logic.handleCreateTransaction(state, 'cust-001', items);

      expect(event.customerId).toBe('cust-001');
      expect(event.subtotalCents).toBe(2000);
      expect(event.items).toEqual(items);
      expect(event.createdAt).toBeDefined();
    });

    it('calculates subtotal for multiple items', () => {
      const state = emptyTransactionState();
      const items: LineItem[] = [
        { productId: 'SKU-001', name: 'Widget', quantity: 2, unitPriceCents: 1000 },
        { productId: 'SKU-002', name: 'Gadget', quantity: 1, unitPriceCents: 2500 },
      ];

      const event = logic.handleCreateTransaction(state, 'cust-002', items);

      expect(event.subtotalCents).toBe(4500);
    });

    it('throws failed_precondition for existing transaction', () => {
      const state: TransactionState = {
        customerId: 'cust-001',
        items: [],
        subtotalCents: 2000,
        discountCents: 0,
        discountType: '',
        status: 'pending',
      };
      const items: LineItem[] = [
        { productId: 'SKU-001', name: 'Widget', quantity: 1, unitPriceCents: 1000 },
      ];

      expect(() => logic.handleCreateTransaction(state, 'cust-002', items)).toThrow(
        CommandValidationError
      );

      try {
        logic.handleCreateTransaction(state, 'cust-002', items);
      } catch (e) {
        expect((e as CommandValidationError).statusCode).toBe(StatusCode.FAILED_PRECONDITION);
      }
    });

    it('throws invalid_argument for empty customer ID', () => {
      const state = emptyTransactionState();
      const items: LineItem[] = [
        { productId: 'SKU-001', name: 'Widget', quantity: 1, unitPriceCents: 1000 },
      ];

      expect(() => logic.handleCreateTransaction(state, '', items)).toThrow(
        CommandValidationError
      );

      try {
        logic.handleCreateTransaction(state, '', items);
      } catch (e) {
        expect((e as CommandValidationError).statusCode).toBe(StatusCode.INVALID_ARGUMENT);
      }
    });

    it('throws invalid_argument for empty items', () => {
      const state = emptyTransactionState();

      expect(() => logic.handleCreateTransaction(state, 'cust-001', [])).toThrow(
        CommandValidationError
      );

      try {
        logic.handleCreateTransaction(state, 'cust-001', []);
      } catch (e) {
        expect((e as CommandValidationError).statusCode).toBe(StatusCode.INVALID_ARGUMENT);
      }
    });
  });

  describe('handleApplyDiscount', () => {
    const pendingState: TransactionState = {
      customerId: 'cust-001',
      items: [],
      subtotalCents: 2000,
      discountCents: 0,
      discountType: '',
      status: 'pending',
    };

    it('applies percentage discount correctly', () => {
      const event = logic.handleApplyDiscount(pendingState, 'percentage', 10);

      expect(event.discountType).toBe('percentage');
      expect(event.value).toBe(10);
      expect(event.discountCents).toBe(200); // 10% of 2000
    });

    it('applies fixed discount correctly', () => {
      const event = logic.handleApplyDiscount(pendingState, 'fixed', 500);

      expect(event.discountType).toBe('fixed');
      expect(event.discountCents).toBe(500);
    });

    it('includes coupon code when provided', () => {
      const event = logic.handleApplyDiscount(pendingState, 'percentage', 10, 'SAVE10');

      expect(event.couponCode).toBe('SAVE10');
    });

    it('throws failed_precondition for non-pending transaction', () => {
      const completedState: TransactionState = {
        ...pendingState,
        status: 'completed',
      };

      expect(() => logic.handleApplyDiscount(completedState, 'percentage', 10)).toThrow(
        CommandValidationError
      );

      try {
        logic.handleApplyDiscount(completedState, 'percentage', 10);
      } catch (e) {
        expect((e as CommandValidationError).statusCode).toBe(StatusCode.FAILED_PRECONDITION);
      }
    });
  });

  describe('handleCompleteTransaction', () => {
    const pendingState: TransactionState = {
      customerId: 'cust-001',
      items: [],
      subtotalCents: 2000,
      discountCents: 0,
      discountType: '',
      status: 'pending',
    };

    it('returns TransactionCompleted event', () => {
      const event = logic.handleCompleteTransaction(pendingState, 'card');

      expect(event.finalTotalCents).toBe(2000);
      expect(event.paymentMethod).toBe('card');
      expect(event.loyaltyPointsEarned).toBe(20); // 1 point per dollar
      expect(event.completedAt).toBeDefined();
    });

    it('calculates final total with discount', () => {
      const stateWithDiscount: TransactionState = {
        ...pendingState,
        discountCents: 200,
      };

      const event = logic.handleCompleteTransaction(stateWithDiscount, 'cash');

      expect(event.finalTotalCents).toBe(1800);
      expect(event.loyaltyPointsEarned).toBe(18);
    });

    it('throws failed_precondition for non-pending transaction', () => {
      const emptyState = emptyTransactionState();

      expect(() => logic.handleCompleteTransaction(emptyState, 'card')).toThrow(
        CommandValidationError
      );

      try {
        logic.handleCompleteTransaction(emptyState, 'card');
      } catch (e) {
        expect((e as CommandValidationError).statusCode).toBe(StatusCode.FAILED_PRECONDITION);
      }
    });
  });

  describe('handleCancelTransaction', () => {
    const pendingState: TransactionState = {
      customerId: 'cust-001',
      items: [],
      subtotalCents: 2000,
      discountCents: 0,
      discountType: '',
      status: 'pending',
    };

    it('returns TransactionCancelled event', () => {
      const event = logic.handleCancelTransaction(pendingState, 'customer request');

      expect(event.reason).toBe('customer request');
      expect(event.cancelledAt).toBeDefined();
    });

    it('throws failed_precondition for completed transaction', () => {
      const completedState: TransactionState = {
        ...pendingState,
        status: 'completed',
      };

      expect(() => logic.handleCancelTransaction(completedState, 'too late')).toThrow(
        CommandValidationError
      );

      try {
        logic.handleCancelTransaction(completedState, 'too late');
      } catch (e) {
        expect((e as CommandValidationError).statusCode).toBe(StatusCode.FAILED_PRECONDITION);
      }
    });
  });
});
