package dev.angzarr.examples.saga.cancellation

import dev.angzarr.*
import com.google.protobuf.Any
import com.google.protobuf.Empty
import examples.Domains.*
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.protobuf.services.HealthStatusManager
import org.slf4j.LoggerFactory

private val logger = LoggerFactory.getLogger("CancellationSaga")
private const val SAGA_NAME = "cancellation"
private const val SOURCE_DOMAIN = "order"

class CancellationSagaService : SagaGrpcKt.SagaCoroutineImplBase() {

    override suspend fun handle(request: EventBook): Empty {
        processEvents(request)
        return Empty.getDefaultInstance()
    }

    override suspend fun handleSync(request: EventBook): SagaResponse {
        val commands = processEvents(request)
        return SagaResponse.newBuilder()
            .addAllCommands(commands)
            .build()
    }

    private fun processEvents(eventBook: EventBook): List<CommandBook> {
        val commands = mutableListOf<CommandBook>()

        if (eventBook.pagesList.isEmpty()) {
            return commands
        }

        for (page in eventBook.pagesList) {
            val event = page.event ?: continue
            val typeUrl = event.typeUrl

            if (!typeUrl.endsWith("OrderCancelled")) continue

            val cancelledEvent = event.unpack(OrderCancelled::class.java)

            val orderId = eventBook.cover?.root?.value?.toByteArray()
                ?.joinToString("") { "%02x".format(it) } ?: ""

            if (orderId.isEmpty()) continue

            logger.info("processing_order_cancellation order_id={}", orderId)

            val releaseCmd = ReleaseReservation.newBuilder()
                .setOrderId(orderId)
                .build()

            val releaseCmdBook = CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                    .setDomain("inventory")
                    .setRoot(eventBook.cover.root)
                    .build())
                .addPages(CommandPage.newBuilder()
                    .setSequence(0)
                    .setSynchronous(false)
                    .setCommand(Any.pack(releaseCmd))
                    .build())
                .setCorrelationId(eventBook.correlationId)
                .build()

            commands.add(releaseCmdBook)

            val loyaltyPointsUsed = cancelledEvent.loyaltyPointsUsed
            if (loyaltyPointsUsed > 0) {
                val addPointsCmd = AddLoyaltyPoints.newBuilder()
                    .setPoints(loyaltyPointsUsed)
                    .setReason("Order cancellation refund")
                    .build()

                val addPointsCmdBook = CommandBook.newBuilder()
                    .setCover(Cover.newBuilder()
                        .setDomain("customer")
                        .build())
                    .addPages(CommandPage.newBuilder()
                        .setSequence(0)
                        .setSynchronous(false)
                        .setCommand(Any.pack(addPointsCmd))
                        .build())
                    .setCorrelationId(eventBook.correlationId)
                    .build()

                commands.add(addPointsCmdBook)
            }
        }

        if (commands.isNotEmpty()) {
            logger.info("processed_cancellation compensation_commands={}", commands.size)
        }

        return commands
    }
}

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50509

    val service = CancellationSagaService()
    val health = HealthStatusManager()

    val server: Server = ServerBuilder.forPort(port)
        .addService(service)
        .addService(health.healthService)
        .build()
        .start()

    health.setStatus("", HealthCheckResponse.ServingStatus.SERVING)
    logger.info("Saga server started: saga={}, port={}, source_domain={}", SAGA_NAME, port, SOURCE_DOMAIN)

    Runtime.getRuntime().addShutdownHook(Thread {
        server.shutdown()
    })

    server.awaitTermination()
}
