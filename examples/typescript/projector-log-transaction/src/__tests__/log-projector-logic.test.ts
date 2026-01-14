import { describe, it, expect, beforeEach } from 'vitest';
import { LogProjectorLogic, EventBook } from '../log-projector-logic.js';

describe('LogProjectorLogic', () => {
  let logic: LogProjectorLogic;

  beforeEach(() => {
    logic = new LogProjectorLogic();
  });

  describe('processEvents', () => {
    it('returns empty array for null event book', () => {
      const entries = logic.processEvents(null);

      expect(entries).toEqual([]);
    });

    it('returns empty array for empty event book', () => {
      const eventBook: EventBook = { pages: [] };

      const entries = logic.processEvents(eventBook);

      expect(entries).toEqual([]);
    });

    it('processes TransactionCreated event', () => {
      const eventBook: EventBook = {
        cover: { domain: 'transaction' },
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCreated',
            data: {
              customerId: 'cust-001',
              items: [{ name: 'Widget' }, { name: 'Gadget' }],
              subtotalCents: 3000,
            },
          },
        ],
      };

      const entries = logic.processEvents(eventBook);

      expect(entries).toHaveLength(1);
      expect(entries[0].eventType).toBe('TransactionCreated');
      expect(entries[0].domain).toBe('transaction');
      expect(entries[0].fields.customerId).toBe('cust-001');
      expect(entries[0].fields.itemCount).toBe(2);
      expect(entries[0].fields.subtotalCents).toBe(3000);
    });

    it('processes DiscountApplied event', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.DiscountApplied',
            data: { discountType: 'percentage', value: 10, discountCents: 300 },
          },
        ],
      };

      const entries = logic.processEvents(eventBook);

      expect(entries).toHaveLength(1);
      expect(entries[0].eventType).toBe('DiscountApplied');
      expect(entries[0].fields.discountType).toBe('percentage');
      expect(entries[0].fields.value).toBe(10);
      expect(entries[0].fields.discountCents).toBe(300);
    });

    it('processes TransactionCompleted event', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCompleted',
            data: { finalTotalCents: 2700, paymentMethod: 'card', loyaltyPointsEarned: 27 },
          },
        ],
      };

      const entries = logic.processEvents(eventBook);

      expect(entries).toHaveLength(1);
      expect(entries[0].eventType).toBe('TransactionCompleted');
      expect(entries[0].fields.finalTotalCents).toBe(2700);
      expect(entries[0].fields.paymentMethod).toBe('card');
      expect(entries[0].fields.loyaltyPointsEarned).toBe(27);
    });

    it('processes TransactionCancelled event', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCancelled',
            data: { reason: 'customer request' },
          },
        ],
      };

      const entries = logic.processEvents(eventBook);

      expect(entries).toHaveLength(1);
      expect(entries[0].eventType).toBe('TransactionCancelled');
      expect(entries[0].fields.reason).toBe('customer request');
    });

    it('processes unknown event type gracefully', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.UnknownEvent',
            data: { foo: 'bar' },
          },
        ],
      };

      const entries = logic.processEvents(eventBook);

      expect(entries).toHaveLength(1);
      expect(entries[0].eventType).toBe('UnknownEvent');
      expect(entries[0].fields.unknown).toBe(true);
    });

    it('includes sequence numbers', () => {
      const eventBook: EventBook = {
        pages: [
          {
            typeUrl: 'type.examples/examples.TransactionCreated',
            data: { customerId: 'cust-001', items: [], subtotalCents: 2000 },
          },
          {
            typeUrl: 'type.examples/examples.TransactionCompleted',
            data: { finalTotalCents: 2000, paymentMethod: 'card' },
          },
        ],
      };

      const entries = logic.processEvents(eventBook);

      expect(entries).toHaveLength(2);
      expect(entries[0].sequence).toBe(0);
      expect(entries[1].sequence).toBe(1);
    });
  });
});
