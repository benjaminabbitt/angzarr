/**
 * Pure business logic for customer aggregate.
 * No gRPC dependencies - can be tested in isolation.
 */
import { CommandValidationError, } from './command-validation-error.js';
import { emptyCustomerState, customerExists, } from './customer-state.js';
/**
 * Customer business logic operations.
 */
export class CustomerLogic {
    /**
     * Rebuilds customer state from an event history.
     */
    rebuildState(eventBook) {
        let state = emptyCustomerState();
        if (!eventBook?.pages?.length) {
            return state;
        }
        // Start from snapshot if present
        if (eventBook.snapshot?.state) {
            const snapState = eventBook.snapshot.state;
            state = {
                name: snapState.name || '',
                email: snapState.email || '',
                loyaltyPoints: snapState.loyaltyPoints || 0,
                lifetimePoints: snapState.lifetimePoints || 0,
            };
        }
        // Apply events
        for (const page of eventBook.pages) {
            state = this.applyEvent(state, page);
        }
        return state;
    }
    applyEvent(state, page) {
        const typeUrl = page.typeUrl || '';
        if (typeUrl.endsWith('CustomerCreated')) {
            return {
                ...state,
                name: page.data.name,
                email: page.data.email,
            };
        }
        else if (typeUrl.endsWith('LoyaltyPointsAdded')) {
            return {
                ...state,
                loyaltyPoints: page.data.newBalance,
                lifetimePoints: state.lifetimePoints + page.data.points,
            };
        }
        else if (typeUrl.endsWith('LoyaltyPointsRedeemed')) {
            return {
                ...state,
                loyaltyPoints: page.data.newBalance,
            };
        }
        return state;
    }
    /**
     * Handles a CreateCustomer command.
     */
    handleCreateCustomer(state, name, email) {
        if (customerExists(state)) {
            throw CommandValidationError.failedPrecondition('Customer already exists');
        }
        if (!name) {
            throw CommandValidationError.invalidArgument('Customer name is required');
        }
        if (!email) {
            throw CommandValidationError.invalidArgument('Customer email is required');
        }
        return {
            name,
            email,
            createdAt: this.now(),
        };
    }
    /**
     * Handles an AddLoyaltyPoints command.
     */
    handleAddLoyaltyPoints(state, points, reason) {
        if (!customerExists(state)) {
            throw CommandValidationError.failedPrecondition('Customer does not exist');
        }
        if (points <= 0) {
            throw CommandValidationError.invalidArgument('Points must be positive');
        }
        const newBalance = state.loyaltyPoints + points;
        return {
            points,
            newBalance,
            reason: reason || '',
        };
    }
    /**
     * Handles a RedeemLoyaltyPoints command.
     */
    handleRedeemLoyaltyPoints(state, points, redemptionType) {
        if (!customerExists(state)) {
            throw CommandValidationError.failedPrecondition('Customer does not exist');
        }
        if (points <= 0) {
            throw CommandValidationError.invalidArgument('Points must be positive');
        }
        if (points > state.loyaltyPoints) {
            throw CommandValidationError.failedPrecondition(`Insufficient points: have ${state.loyaltyPoints}, need ${points}`);
        }
        const newBalance = state.loyaltyPoints - points;
        return {
            points,
            newBalance,
            redemptionType: redemptionType || '',
        };
    }
    now() {
        return { seconds: Math.floor(Date.now() / 1000), nanos: 0 };
    }
}
//# sourceMappingURL=customer-logic.js.map