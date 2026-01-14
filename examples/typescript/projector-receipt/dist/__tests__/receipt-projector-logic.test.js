import { describe, it, expect, beforeEach } from 'vitest';
import { ReceiptProjectorLogic } from '../receipt-projector-logic.js';
import { emptyReceiptState } from '../receipt-state.js';
describe('ReceiptProjectorLogic', () => {
    let logic;
    beforeEach(() => {
        logic = new ReceiptProjectorLogic();
    });
    describe('buildReceiptState', () => {
        it('returns empty state for null event book', () => {
            const state = logic.buildReceiptState(null);
            expect(state.customerId).toBe('');
            expect(state.isCompleted).toBe(false);
        });
        it('returns empty state for empty event book', () => {
            const eventBook = { pages: [] };
            const state = logic.buildReceiptState(eventBook);
            expect(state.isCompleted).toBe(false);
        });
        it('applies TransactionCreated event', () => {
            const items = [
                { productId: 'SKU-001', name: 'Widget', quantity: 2, unitPriceCents: 1000 },
            ];
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-001', items, subtotalCents: 2000 },
                    },
                ],
            };
            const state = logic.buildReceiptState(eventBook);
            expect(state.customerId).toBe('cust-001');
            expect(state.items).toEqual(items);
            expect(state.subtotalCents).toBe(2000);
            expect(state.isCompleted).toBe(false);
        });
        it('applies DiscountApplied event', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-001', items: [], subtotalCents: 2000 },
                    },
                    {
                        typeUrl: 'type.examples/examples.DiscountApplied',
                        data: { discountCents: 200 },
                    },
                ],
            };
            const state = logic.buildReceiptState(eventBook);
            expect(state.discountCents).toBe(200);
        });
        it('applies TransactionCompleted event', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-001', items: [], subtotalCents: 2000 },
                    },
                    {
                        typeUrl: 'type.examples/examples.TransactionCompleted',
                        data: {
                            finalTotalCents: 2000,
                            paymentMethod: 'card',
                            loyaltyPointsEarned: 20,
                        },
                    },
                ],
            };
            const state = logic.buildReceiptState(eventBook);
            expect(state.isCompleted).toBe(true);
            expect(state.finalTotalCents).toBe(2000);
            expect(state.paymentMethod).toBe('card');
            expect(state.loyaltyPointsEarned).toBe(20);
        });
    });
    describe('project', () => {
        it('returns null for incomplete transaction', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-001', items: [], subtotalCents: 2000 },
                    },
                ],
            };
            const receipt = logic.project(eventBook);
            expect(receipt).toBeNull();
        });
        it('returns receipt for completed transaction', () => {
            const items = [
                { productId: 'SKU-001', name: 'Widget', quantity: 2, unitPriceCents: 1000 },
            ];
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-001', items, subtotalCents: 2000 },
                    },
                    {
                        typeUrl: 'type.examples/examples.TransactionCompleted',
                        data: {
                            finalTotalCents: 2000,
                            paymentMethod: 'card',
                            loyaltyPointsEarned: 20,
                        },
                    },
                ],
            };
            const receipt = logic.project(eventBook);
            expect(receipt).not.toBeNull();
            expect(receipt.customerId).toBe('cust-001');
            expect(receipt.finalTotalCents).toBe(2000);
            expect(receipt.paymentMethod).toBe('card');
        });
        it('includes discount in receipt', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-002', items: [], subtotalCents: 2000 },
                    },
                    {
                        typeUrl: 'type.examples/examples.DiscountApplied',
                        data: { discountCents: 200 },
                    },
                    {
                        typeUrl: 'type.examples/examples.TransactionCompleted',
                        data: {
                            finalTotalCents: 1800,
                            paymentMethod: 'cash',
                            loyaltyPointsEarned: 18,
                        },
                    },
                ],
            };
            const receipt = logic.project(eventBook);
            expect(receipt).not.toBeNull();
            expect(receipt.subtotalCents).toBe(2000);
            expect(receipt.discountCents).toBe(200);
            expect(receipt.finalTotalCents).toBe(1800);
        });
        it('includes loyalty points in receipt', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.TransactionCreated',
                        data: { customerId: 'cust-003', items: [], subtotalCents: 5000 },
                    },
                    {
                        typeUrl: 'type.examples/examples.TransactionCompleted',
                        data: {
                            finalTotalCents: 5000,
                            paymentMethod: 'card',
                            loyaltyPointsEarned: 50,
                        },
                    },
                ],
            };
            const receipt = logic.project(eventBook);
            expect(receipt).not.toBeNull();
            expect(receipt.loyaltyPointsEarned).toBe(50);
        });
    });
    describe('formatReceipt', () => {
        it('includes RECEIPT header', () => {
            const state = {
                ...emptyReceiptState(),
                isCompleted: true,
                finalTotalCents: 1000,
            };
            const text = logic.formatReceipt(state);
            expect(text).toContain('RECEIPT');
        });
        it('includes item details', () => {
            const state = {
                ...emptyReceiptState(),
                items: [
                    { productId: 'SKU-001', name: 'Widget', quantity: 1, unitPriceCents: 1000 },
                ],
                isCompleted: true,
                finalTotalCents: 1000,
            };
            const text = logic.formatReceipt(state);
            expect(text).toContain('Widget');
        });
        it('includes Thank you message', () => {
            const state = {
                ...emptyReceiptState(),
                isCompleted: true,
                finalTotalCents: 1000,
            };
            const text = logic.formatReceipt(state);
            expect(text).toContain('Thank you');
        });
        it('includes discount when present', () => {
            const state = {
                ...emptyReceiptState(),
                subtotalCents: 2000,
                discountCents: 200,
                finalTotalCents: 1800,
                isCompleted: true,
            };
            const text = logic.formatReceipt(state);
            expect(text).toContain('Discount');
            expect(text).toContain('$2.00');
        });
        it('includes loyalty points when earned', () => {
            const state = {
                ...emptyReceiptState(),
                finalTotalCents: 2000,
                loyaltyPointsEarned: 20,
                isCompleted: true,
            };
            const text = logic.formatReceipt(state);
            expect(text).toContain('Loyalty Points Earned: 20');
        });
    });
});
//# sourceMappingURL=receipt-projector-logic.test.js.map