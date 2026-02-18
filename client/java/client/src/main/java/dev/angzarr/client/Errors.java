package dev.angzarr.client;

import io.grpc.Status;

/**
 * Exception types for Angzarr client errors.
 */
public class Errors {

    /**
     * Base exception for all Angzarr client errors.
     */
    public static class ClientError extends RuntimeException {
        public ClientError(String message) {
            super(message);
        }

        public ClientError(String message, Throwable cause) {
            super(message, cause);
        }
    }

    /**
     * Thrown when a command is rejected due to business rule violation.
     *
     * <p>This exception maps to gRPC status codes:
     * <ul>
     *   <li>{@link Status#FAILED_PRECONDITION} - State precondition not met (e.g., "Player already exists")
     *   <li>{@link Status#INVALID_ARGUMENT} - Invalid command input (e.g., "amount must be positive")
     * </ul>
     *
     * <p>Usage:
     * <pre>{@code
     * if (state.exists()) {
     *     throw Errors.CommandRejectedError.preconditionFailed("Player already exists");
     * }
     * if (amount <= 0) {
     *     throw Errors.CommandRejectedError.invalidArgument("amount must be positive");
     * }
     * }</pre>
     */
    public static class CommandRejectedError extends ClientError {
        private final Status.Code statusCode;

        public CommandRejectedError(String message) {
            this(message, Status.Code.FAILED_PRECONDITION);
        }

        public CommandRejectedError(String message, Status.Code statusCode) {
            super(message);
            this.statusCode = statusCode;
        }

        public Status.Code getStatusCode() {
            return statusCode;
        }

        /**
         * Create a FAILED_PRECONDITION error for state precondition violations.
         */
        public static CommandRejectedError preconditionFailed(String message) {
            return new CommandRejectedError(message, Status.Code.FAILED_PRECONDITION);
        }

        /**
         * Create an INVALID_ARGUMENT error for invalid command inputs.
         */
        public static CommandRejectedError invalidArgument(String message) {
            return new CommandRejectedError(message, Status.Code.INVALID_ARGUMENT);
        }

        /**
         * Convert to gRPC Status for RPC responses.
         */
        public Status toGrpcStatus() {
            return Status.fromCode(statusCode).withDescription(getMessage());
        }
    }

    /**
     * Thrown when a gRPC call fails.
     */
    public static class GrpcError extends ClientError {
        private final Status.Code statusCode;

        public GrpcError(String message, Status.Code statusCode) {
            super(message);
            this.statusCode = statusCode;
        }

        public Status.Code getStatusCode() {
            return statusCode;
        }
    }

    /**
     * Thrown when connection to the server fails.
     */
    public static class ConnectionError extends ClientError {
        public ConnectionError(String message) {
            super(message);
        }

        public ConnectionError(String message, Throwable cause) {
            super(message, cause);
        }
    }

    /**
     * Thrown when transport-level errors occur.
     */
    public static class TransportError extends ClientError {
        public TransportError(String message) {
            super(message);
        }

        public TransportError(String message, Throwable cause) {
            super(message, cause);
        }
    }

    /**
     * Thrown when an invalid argument is provided.
     */
    public static class InvalidArgumentError extends ClientError {
        public InvalidArgumentError(String message) {
            super(message);
        }
    }

    /**
     * Thrown when a timestamp cannot be parsed.
     */
    public static class InvalidTimestampError extends ClientError {
        public InvalidTimestampError(String message) {
            super(message);
        }
    }
}
