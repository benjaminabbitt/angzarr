/**
 * Loyalty Saga - Kotlin Implementation
 *
 * Awards loyalty points when transactions complete.
 */
package dev.angzarr.examples.saga

import dev.angzarr.EventBook
import dev.angzarr.SagaGrpcKt
import dev.angzarr.SagaResponse
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.protobuf.services.HealthStatusManager
import org.slf4j.LoggerFactory

private val logger = LoggerFactory.getLogger("LoyaltySaga")
private const val SAGA_NAME = "loyalty"

/**
 * gRPC service adapter for loyalty saga.
 */
class LoyaltySagaService(
    private val logic: LoyaltySagaLogic
) : SagaGrpcKt.SagaCoroutineImplBase() {

    override suspend fun handle(request: EventBook): com.google.protobuf.Empty {
        return com.google.protobuf.Empty.getDefaultInstance()
    }

    override suspend fun handleSync(request: EventBook): SagaResponse {
        val commands = logic.processEvents(request)
        return SagaResponse.newBuilder()
            .addAllCommands(commands)
            .build()
    }
}

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50054

    val logic: LoyaltySagaLogic = DefaultLoyaltySagaLogic()
    val service = LoyaltySagaService(logic)
    val health = HealthStatusManager()

    val server: Server = ServerBuilder.forPort(port)
        .addService(service)
        .addService(health.healthService)
        .build()
        .start()

    health.setStatus("", HealthCheckResponse.ServingStatus.SERVING)
    logger.info("Saga server started: name={}, port={}", SAGA_NAME, port)

    Runtime.getRuntime().addShutdownHook(Thread {
        server.shutdown()
    })

    server.awaitTermination()
}
