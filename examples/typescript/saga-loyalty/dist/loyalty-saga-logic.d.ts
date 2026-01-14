/**
 * Pure business logic for loyalty saga.
 * No gRPC dependencies - can be tested in isolation.
 */
export interface EventPage {
    typeUrl: string;
    data: any;
}
export interface EventBook {
    cover?: {
        domain?: string;
        root?: {
            value?: Uint8Array;
        };
    };
    pages: EventPage[];
}
export interface AddLoyaltyPointsCommand {
    domain: string;
    customerId: string;
    points: number;
    reason: string;
}
/**
 * Loyalty saga business logic.
 */
export declare class LoyaltySagaLogic {
    /**
     * Processes events and generates commands.
     */
    process(eventBook: EventBook | null | undefined): AddLoyaltyPointsCommand[];
    private uuidToHex;
}
