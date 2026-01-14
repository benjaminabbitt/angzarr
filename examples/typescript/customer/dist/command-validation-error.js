/**
 * Error class for command validation failures.
 * Bridges domain validation to gRPC status codes without coupling logic to gRPC.
 */
export var StatusCode;
(function (StatusCode) {
    StatusCode["INVALID_ARGUMENT"] = "INVALID_ARGUMENT";
    StatusCode["FAILED_PRECONDITION"] = "FAILED_PRECONDITION";
})(StatusCode || (StatusCode = {}));
export class CommandValidationError extends Error {
    statusCode;
    constructor(message, statusCode) {
        super(message);
        this.statusCode = statusCode;
        this.name = 'CommandValidationError';
    }
    static invalidArgument(message) {
        return new CommandValidationError(message, StatusCode.INVALID_ARGUMENT);
    }
    static failedPrecondition(message) {
        return new CommandValidationError(message, StatusCode.FAILED_PRECONDITION);
    }
}
//# sourceMappingURL=command-validation-error.js.map