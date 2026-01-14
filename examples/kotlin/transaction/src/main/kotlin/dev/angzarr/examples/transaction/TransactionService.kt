package dev.angzarr.examples.transaction

import dev.angzarr.BusinessLogicGrpcKt
import dev.angzarr.BusinessResponse
import dev.angzarr.ContextualCommand
import examples.Domains.*
import io.grpc.Status

/**
 * gRPC service adapter for transaction business logic.
 * Thin wrapper that delegates to TransactionLogic and maps exceptions to gRPC status codes.
 */
class TransactionService(
    private val logic: TransactionLogic
) : BusinessLogicGrpcKt.BusinessLogicCoroutineImplBase() {

    override suspend fun handle(request: ContextualCommand): BusinessResponse {
        val cmdBook = request.command
        val priorEvents = request.events

        if (cmdBook.pagesList.isEmpty()) {
            throw Status.INVALID_ARGUMENT
                .withDescription("CommandBook has no pages")
                .asRuntimeException()
        }

        val cmdPage = cmdBook.pagesList[0]
        val cmd = cmdPage.command ?: throw Status.INVALID_ARGUMENT
            .withDescription("Command page has no command")
            .asRuntimeException()

        val state = logic.rebuildState(priorEvents)

        try {
            val eventBook = when {
                cmd.`is`(CreateTransaction::class.java) -> {
                    val c = cmd.unpack(CreateTransaction::class.java)
                    logic.handleCreateTransaction(state, c.customerId, c.itemsList)
                }
                cmd.`is`(ApplyDiscount::class.java) -> {
                    val c = cmd.unpack(ApplyDiscount::class.java)
                    logic.handleApplyDiscount(state, c.discountType, c.value, c.couponCode)
                }
                cmd.`is`(CompleteTransaction::class.java) -> {
                    val c = cmd.unpack(CompleteTransaction::class.java)
                    logic.handleCompleteTransaction(state, c.paymentMethod)
                }
                cmd.`is`(CancelTransaction::class.java) -> {
                    val c = cmd.unpack(CancelTransaction::class.java)
                    logic.handleCancelTransaction(state, c.reason)
                }
                else -> throw Status.INVALID_ARGUMENT
                    .withDescription("Unknown command type: ${cmd.typeUrl}")
                    .asRuntimeException()
            }

            val eventBookWithCover = eventBook.toBuilder()
                .setCover(cmdBook.cover)
                .build()

            return BusinessResponse.newBuilder()
                .setEvents(eventBookWithCover)
                .build()

        } catch (e: CommandValidationException) {
            throw Status.fromCode(e.statusCode)
                .withDescription(e.message)
                .asRuntimeException()
        }
    }
}
