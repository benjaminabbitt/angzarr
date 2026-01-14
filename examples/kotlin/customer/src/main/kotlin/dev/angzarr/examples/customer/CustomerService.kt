package dev.angzarr.examples.customer

import dev.angzarr.BusinessLogicGrpcKt
import dev.angzarr.BusinessResponse
import dev.angzarr.ContextualCommand
import examples.Domains.AddLoyaltyPoints
import examples.Domains.CreateCustomer
import examples.Domains.RedeemLoyaltyPoints
import io.grpc.Status

/**
 * gRPC service adapter for customer business logic.
 * Thin wrapper that delegates to CustomerLogic and maps exceptions to gRPC status codes.
 */
class CustomerService(
    private val logic: CustomerLogic
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
                cmd.`is`(CreateCustomer::class.java) -> {
                    val c = cmd.unpack(CreateCustomer::class.java)
                    logic.handleCreateCustomer(state, c.name, c.email)
                }
                cmd.`is`(AddLoyaltyPoints::class.java) -> {
                    val c = cmd.unpack(AddLoyaltyPoints::class.java)
                    logic.handleAddLoyaltyPoints(state, c.points, c.reason)
                }
                cmd.`is`(RedeemLoyaltyPoints::class.java) -> {
                    val c = cmd.unpack(RedeemLoyaltyPoints::class.java)
                    logic.handleRedeemLoyaltyPoints(state, c.points, c.redemptionType)
                }
                else -> throw Status.INVALID_ARGUMENT
                    .withDescription("Unknown command type: ${cmd.typeUrl}")
                    .asRuntimeException()
            }

            // Copy cover from command book to event book
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
