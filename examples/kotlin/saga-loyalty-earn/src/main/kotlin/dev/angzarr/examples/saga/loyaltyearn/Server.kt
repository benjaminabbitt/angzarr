package dev.angzarr.examples.saga.loyaltyearn

import dev.angzarr.*
import com.google.protobuf.Any
import com.google.protobuf.Empty
import examples.Domains.*
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.protobuf.services.HealthStatusManager
import org.slf4j.LoggerFactory

private val logger = LoggerFactory.getLogger("LoyaltyEarnSaga")
private const val SAGA_NAME = "loyalty-earn"
private const val SOURCE_DOMAIN = "fulfillment"
private const val POINTS_PER_DOLLAR = 10

class LoyaltyEarnSagaService : SagaGrpcKt.SagaCoroutineImplBase() {

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

            if (!typeUrl.endsWith("Delivered")) continue

            val orderId = eventBook.cover?.root?.value?.toByteArray()
                ?.joinToString("") { "%02x".format(it) } ?: ""

            if (orderId.isEmpty()) continue

            val points = POINTS_PER_DOLLAR * 100

            logger.info("awarding_loyalty_points order_id={} points={}", orderId, points)

            val addPointsCmd = AddLoyaltyPoints.newBuilder()
                .setPoints(points)
                .setReason("Order delivery: $orderId")
                .build()

            val cmdBook = CommandBook.newBuilder()
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

            commands.add(cmdBook)
        }

        if (commands.isNotEmpty()) {
            logger.info("processed_delivery_for_loyalty command_count={}", commands.size)
        }

        return commands
    }
}

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50508

    val service = LoyaltyEarnSagaService()
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
