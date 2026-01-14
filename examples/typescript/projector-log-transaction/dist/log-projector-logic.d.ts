/**
 * Pure business logic for transaction log projector.
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
export interface LogEntry {
    domain: string;
    rootId: string;
    sequence: number;
    eventType: string;
    fields: Record<string, any>;
}
/**
 * Transaction log projector business logic.
 */
export declare class LogProjectorLogic {
    /**
     * Processes an event book and returns log entries.
     */
    processEvents(eventBook: EventBook | null | undefined): LogEntry[];
    private uuidToHex;
}
