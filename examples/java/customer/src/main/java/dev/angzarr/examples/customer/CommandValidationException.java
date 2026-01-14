package dev.angzarr.examples.customer;

import io.grpc.Status;

/**
 * Exception thrown when command validation fails.
 */
public class CommandValidationException extends Exception {
    private final Status.Code statusCode;

    public CommandValidationException(Status.Code statusCode, String message) {
        super(message);
        this.statusCode = statusCode;
    }

    public static CommandValidationException invalidArgument(String message) {
        return new CommandValidationException(Status.Code.INVALID_ARGUMENT, message);
    }

    public static CommandValidationException failedPrecondition(String message) {
        return new CommandValidationException(Status.Code.FAILED_PRECONDITION, message);
    }

    public Status.Code getStatusCode() {
        return statusCode;
    }
}
