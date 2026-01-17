package dev.angzarr.examples.fulfillment

import io.grpc.Status

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
