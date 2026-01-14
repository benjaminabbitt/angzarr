import { describe, it, expect, beforeEach } from 'vitest';
import { LogProjectorLogic } from '../log-projector-logic.js';
describe('LogProjectorLogic', () => {
    let logic;
    beforeEach(() => {
        logic = new LogProjectorLogic();
    });
    describe('processEvents', () => {
        it('returns empty array for null event book', () => {
            const entries = logic.processEvents(null);
            expect(entries).toEqual([]);
        });
        it('returns empty array for empty event book', () => {
            const eventBook = { pages: [] };
            const entries = logic.processEvents(eventBook);
            expect(entries).toEqual([]);
        });
        it('processes CustomerCreated event', () => {
            const eventBook = {
                cover: { domain: 'customer' },
                pages: [
                    {
                        typeUrl: 'type.examples/examples.CustomerCreated',
                        data: { name: 'Alice', email: 'alice@example.com' },
                    },
                ],
            };
            const entries = logic.processEvents(eventBook);
            expect(entries).toHaveLength(1);
            expect(entries[0].eventType).toBe('CustomerCreated');
            expect(entries[0].domain).toBe('customer');
            expect(entries[0].fields.name).toBe('Alice');
            expect(entries[0].fields.email).toBe('alice@example.com');
        });
        it('processes LoyaltyPointsAdded event', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.LoyaltyPointsAdded',
                        data: { points: 100, newBalance: 100, reason: 'welcome' },
                    },
                ],
            };
            const entries = logic.processEvents(eventBook);
            expect(entries).toHaveLength(1);
            expect(entries[0].eventType).toBe('LoyaltyPointsAdded');
            expect(entries[0].fields.points).toBe(100);
            expect(entries[0].fields.newBalance).toBe(100);
            expect(entries[0].fields.reason).toBe('welcome');
        });
        it('processes LoyaltyPointsRedeemed event', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.LoyaltyPointsRedeemed',
                        data: { points: 50, newBalance: 50, redemptionType: 'discount' },
                    },
                ],
            };
            const entries = logic.processEvents(eventBook);
            expect(entries).toHaveLength(1);
            expect(entries[0].eventType).toBe('LoyaltyPointsRedeemed');
            expect(entries[0].fields.points).toBe(50);
            expect(entries[0].fields.newBalance).toBe(50);
            expect(entries[0].fields.redemptionType).toBe('discount');
        });
        it('processes unknown event type gracefully', () => {
            const eventBook = {
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
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.CustomerCreated',
                        data: { name: 'Alice', email: 'alice@example.com' },
                    },
                    {
                        typeUrl: 'type.examples/examples.LoyaltyPointsAdded',
                        data: { points: 100, newBalance: 100 },
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
//# sourceMappingURL=log-projector-logic.test.js.map