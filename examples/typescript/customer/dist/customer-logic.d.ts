/**
 * Pure business logic for customer aggregate.
 * No gRPC dependencies - can be tested in isolation.
 */
import { CustomerState } from './customer-state.js';
export interface CustomerCreatedEvent {
    name: string;
    email: string;
    createdAt: {
        seconds: number;
        nanos: number;
    };
}
export interface LoyaltyPointsAddedEvent {
    points: number;
    newBalance: number;
    reason: string;
}
export interface LoyaltyPointsRedeemedEvent {
    points: number;
    newBalance: number;
    redemptionType: string;
}
export type CustomerEvent = {
    type: 'CustomerCreated';
    data: CustomerCreatedEvent;
} | {
    type: 'LoyaltyPointsAdded';
    data: LoyaltyPointsAddedEvent;
} | {
    type: 'LoyaltyPointsRedeemed';
    data: LoyaltyPointsRedeemedEvent;
};
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
 * Customer business logic operations.
 */
export declare class CustomerLogic {
    /**
     * Rebuilds customer state from an event history.
     */
    rebuildState(eventBook: EventBook | null | undefined): CustomerState;
    private applyEvent;
    /**
     * Handles a CreateCustomer command.
     */
    handleCreateCustomer(state: CustomerState, name: string, email: string): CustomerCreatedEvent;
    /**
     * Handles an AddLoyaltyPoints command.
     */
    handleAddLoyaltyPoints(state: CustomerState, points: number, reason: string): LoyaltyPointsAddedEvent;
    /**
     * Handles a RedeemLoyaltyPoints command.
     */
    handleRedeemLoyaltyPoints(state: CustomerState, points: number, redemptionType: string): LoyaltyPointsRedeemedEvent;
    private now;
}
