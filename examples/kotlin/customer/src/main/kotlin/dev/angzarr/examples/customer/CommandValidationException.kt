package dev.angzarr.examples.customer

import io.grpc.Status

/**
 * Exception for command validation failures.
 * Bridges domain validation to gRPC status codes without coupling logic to gRPC.
 */
class CommandValidationException(
    message: String,
    val statusCode: Status.Code
) : Exception(message) {

    companion object {
        fun invalidArgument(message: String): CommandValidationException =
            CommandValidationException(message, Status.Code.INVALID_ARGUMENT)

        fun failedPrecondition(message: String): CommandValidationException =
            CommandValidationException(message, Status.Code.FAILED_PRECONDITION)
    }
}
