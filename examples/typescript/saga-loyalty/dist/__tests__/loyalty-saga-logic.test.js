import { describe, it, expect, beforeEach } from 'vitest';
import { LoyaltySagaLogic } from '../loyalty-saga-logic.js';
describe('LoyaltySagaLogic', () => {
    let logic;
    beforeEach(() => {
        logic = new LoyaltySagaLogic();
    });
    describe('process', () => {
        it('returns empty array for null event book', () => {
            const commands = logic.process(null);
            expect(commands).toEqual([]);
        });
        it('returns empty array for empty event book', () => {
            const eventBook = { pages: [] };
            const commands = logic.process(eventBook);
            expect(commands).toEqual([]);
        });
        it('returns empty array for incomplete transaction', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-001', subtotalCents: 2000 },
                    },
                ],
            };
            const commands = logic.process(eventBook);
            expect(commands).toEqual([]);
        });
        it('generates AddLoyaltyPoints command for completed transaction', () => {
            const eventBook = {
                cover: { root: { value: Buffer.from('12345678') } },
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCompleted',
                        data: { loyaltyPointsEarned: 20 },
                    },
                ],
            };
            const commands = logic.process(eventBook);
            expect(commands).toHaveLength(1);
            expect(commands[0].points).toBe(20);
            expect(commands[0].domain).toBe('customer');
        });
        it('returns empty array for zero loyalty points', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCompleted',
                        data: { loyaltyPointsEarned: 0 },
                    },
                ],
            };
            const commands = logic.process(eventBook);
            expect(commands).toEqual([]);
        });
        it('includes transaction reference in reason', () => {
            const eventBook = {
                cover: { root: { value: Buffer.from('abcd1234') } },
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCompleted',
                        data: { loyaltyPointsEarned: 50 },
                    },
                ],
            };
            const commands = logic.process(eventBook);
            expect(commands).toHaveLength(1);
            expect(commands[0].reason).toContain('transaction');
        });
        it('ignores other event types', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-001' },
                    },
                    {
                        typeUrl: 'type.examples/examples.DiscountApplied',
                        data: { discountCents: 200 },
                    },
                ],
            };
            const commands = logic.process(eventBook);
            expect(commands).toEqual([]);
        });
    });
});
//# sourceMappingURL=loyalty-saga-logic.test.js.map