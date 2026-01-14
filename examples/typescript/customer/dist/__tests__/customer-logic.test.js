import { describe, it, expect, beforeEach } from 'vitest';
import { CustomerLogic } from '../customer-logic.js';
import { emptyCustomerState } from '../customer-state.js';
import { CommandValidationError, StatusCode } from '../command-validation-error.js';
describe('CustomerLogic', () => {
    let logic;
    beforeEach(() => {
        logic = new CustomerLogic();
    });
    describe('rebuildState', () => {
        it('returns empty state for null event book', () => {
            const state = logic.rebuildState(null);
            expect(state.name).toBe('');
            expect(state.email).toBe('');
            expect(state.loyaltyPoints).toBe(0);
            expect(state.lifetimePoints).toBe(0);
        });
        it('returns empty state for empty event book', () => {
            const eventBook = { pages: [] };
            const state = logic.rebuildState(eventBook);
            expect(state.name).toBe('');
        });
        it('applies CustomerCreated event', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.CustomerCreated',
                        data: { name: 'John Doe', email: 'john@example.com' },
                    },
                ],
            };
            const state = logic.rebuildState(eventBook);
            expect(state.name).toBe('John Doe');
            expect(state.email).toBe('john@example.com');
        });
        it('applies LoyaltyPointsAdded event', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.CustomerCreated',
                        data: { name: 'John', email: 'john@example.com' },
                    },
                    {
                        typeUrl: 'type.examples/examples.LoyaltyPointsAdded',
                        data: { points: 100, newBalance: 100, reason: 'welcome' },
                    },
                ],
            };
            const state = logic.rebuildState(eventBook);
            expect(state.loyaltyPoints).toBe(100);
            expect(state.lifetimePoints).toBe(100);
        });
        it('applies LoyaltyPointsRedeemed event', () => {
            const eventBook = {
                pages: [
                    {
                        typeUrl: 'type.examples/examples.CustomerCreated',
                        data: { name: 'John', email: 'john@example.com' },
                    },
                    {
                        typeUrl: 'type.examples/examples.LoyaltyPointsAdded',
                        data: { points: 100, newBalance: 100 },
                    },
                    {
                        typeUrl: 'type.examples/examples.LoyaltyPointsRedeemed',
                        data: { points: 30, newBalance: 70 },
                    },
                ],
            };
            const state = logic.rebuildState(eventBook);
            expect(state.loyaltyPoints).toBe(70);
            expect(state.lifetimePoints).toBe(100); // Lifetime not reduced
        });
    });
    describe('handleCreateCustomer', () => {
        it('returns CustomerCreated event for new customer', () => {
            const state = emptyCustomerState();
            const event = logic.handleCreateCustomer(state, 'Alice', 'alice@example.com');
            expect(event.name).toBe('Alice');
            expect(event.email).toBe('alice@example.com');
            expect(event.createdAt).toBeDefined();
        });
        it('throws failed_precondition for existing customer', () => {
            const state = {
                name: 'Existing',
                email: 'existing@test.com',
                loyaltyPoints: 0,
                lifetimePoints: 0,
            };
            expect(() => logic.handleCreateCustomer(state, 'New', 'new@test.com')).toThrow(CommandValidationError);
            try {
                logic.handleCreateCustomer(state, 'New', 'new@test.com');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.FAILED_PRECONDITION);
                expect(e.message).toContain('already exists');
            }
        });
        it('throws invalid_argument for empty name', () => {
            const state = emptyCustomerState();
            expect(() => logic.handleCreateCustomer(state, '', 'email@test.com')).toThrow(CommandValidationError);
            try {
                logic.handleCreateCustomer(state, '', 'email@test.com');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.INVALID_ARGUMENT);
            }
        });
        it('throws invalid_argument for empty email', () => {
            const state = emptyCustomerState();
            expect(() => logic.handleCreateCustomer(state, 'Name', '')).toThrow(CommandValidationError);
            try {
                logic.handleCreateCustomer(state, 'Name', '');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.INVALID_ARGUMENT);
            }
        });
    });
    describe('handleAddLoyaltyPoints', () => {
        it('returns LoyaltyPointsAdded event', () => {
            const state = {
                name: 'John',
                email: 'john@test.com',
                loyaltyPoints: 50,
                lifetimePoints: 100,
            };
            const event = logic.handleAddLoyaltyPoints(state, 25, 'purchase');
            expect(event.points).toBe(25);
            expect(event.newBalance).toBe(75);
            expect(event.reason).toBe('purchase');
        });
        it('throws failed_precondition for non-existent customer', () => {
            const state = emptyCustomerState();
            expect(() => logic.handleAddLoyaltyPoints(state, 25, 'purchase')).toThrow(CommandValidationError);
            try {
                logic.handleAddLoyaltyPoints(state, 25, 'purchase');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.FAILED_PRECONDITION);
            }
        });
        it('throws invalid_argument for zero points', () => {
            const state = {
                name: 'John',
                email: 'john@test.com',
                loyaltyPoints: 50,
                lifetimePoints: 0,
            };
            expect(() => logic.handleAddLoyaltyPoints(state, 0, 'purchase')).toThrow(CommandValidationError);
            try {
                logic.handleAddLoyaltyPoints(state, 0, 'purchase');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.INVALID_ARGUMENT);
            }
        });
        it('throws invalid_argument for negative points', () => {
            const state = {
                name: 'John',
                email: 'john@test.com',
                loyaltyPoints: 50,
                lifetimePoints: 0,
            };
            expect(() => logic.handleAddLoyaltyPoints(state, -10, 'purchase')).toThrow(CommandValidationError);
        });
    });
    describe('handleRedeemLoyaltyPoints', () => {
        it('returns LoyaltyPointsRedeemed event', () => {
            const state = {
                name: 'John',
                email: 'john@test.com',
                loyaltyPoints: 100,
                lifetimePoints: 200,
            };
            const event = logic.handleRedeemLoyaltyPoints(state, 50, 'discount');
            expect(event.points).toBe(50);
            expect(event.newBalance).toBe(50);
            expect(event.redemptionType).toBe('discount');
        });
        it('throws failed_precondition for non-existent customer', () => {
            const state = emptyCustomerState();
            expect(() => logic.handleRedeemLoyaltyPoints(state, 50, 'discount')).toThrow(CommandValidationError);
            try {
                logic.handleRedeemLoyaltyPoints(state, 50, 'discount');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.FAILED_PRECONDITION);
            }
        });
        it('throws failed_precondition for insufficient points', () => {
            const state = {
                name: 'John',
                email: 'john@test.com',
                loyaltyPoints: 30,
                lifetimePoints: 0,
            };
            expect(() => logic.handleRedeemLoyaltyPoints(state, 50, 'discount')).toThrow(CommandValidationError);
            try {
                logic.handleRedeemLoyaltyPoints(state, 50, 'discount');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.FAILED_PRECONDITION);
                expect(e.message).toContain('Insufficient');
            }
        });
        it('throws invalid_argument for zero points', () => {
            const state = {
                name: 'John',
                email: 'john@test.com',
                loyaltyPoints: 100,
                lifetimePoints: 0,
            };
            expect(() => logic.handleRedeemLoyaltyPoints(state, 0, 'discount')).toThrow(CommandValidationError);
            try {
                logic.handleRedeemLoyaltyPoints(state, 0, 'discount');
            }
            catch (e) {
                expect(e.statusCode).toBe(StatusCode.INVALID_ARGUMENT);
            }
        });
    });
});
//# sourceMappingURL=customer-logic.test.js.map