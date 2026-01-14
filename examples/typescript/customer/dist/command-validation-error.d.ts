/**
 * Error class for command validation failures.
 * Bridges domain validation to gRPC status codes without coupling logic to gRPC.
 */
export declare enum StatusCode {
    INVALID_ARGUMENT = "INVALID_ARGUMENT",
    FAILED_PRECONDITION = "FAILED_PRECONDITION"
}
export declare class CommandValidationError extends Error {
    readonly statusCode: StatusCode;
    constructor(message: string, statusCode: StatusCode);
    static invalidArgument(message: string): CommandValidationError;
    static failedPrecondition(message: string): CommandValidationError;
}
