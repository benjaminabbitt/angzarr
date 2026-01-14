/**
 * Receipt Projector - Kotlin Implementation
 *
 * Projects completed transactions into receipt documents.
 */
package dev.angzarr.examples.projector

import dev.angzarr.EventBook
import dev.angzarr.ProjectorCoordinatorGrpcKt
import dev.angzarr.Projection
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.protobuf.services.HealthStatusManager
import org.slf4j.LoggerFactory

private val logger = LoggerFactory.getLogger("ReceiptProjector")
private const val PROJECTOR_NAME = "receipt"

/**
 * gRPC service adapter for receipt projector.
 */
class ReceiptProjectorService(
    private val logic: ReceiptProjectorLogic
) : ProjectorCoordinatorGrpcKt.ProjectorCoordinatorCoroutineImplBase() {

    override suspend fun handle(request: EventBook): com.google.protobuf.Empty {
        return com.google.protobuf.Empty.getDefaultInstance()
    }

    override suspend fun handleSync(request: EventBook): Projection {
        val projection = logic.createProjection(request, PROJECTOR_NAME)

        if (projection != null) {
            val transactionId = request.cover?.root?.value?.toByteArray()
                ?.joinToString("") { "%02x".format(it) }?.take(16) ?: ""
            val state = logic.buildState(request)
            logger.info("generated_receipt transaction={} total={}", transactionId, state.finalTotalCents)
        }

        return projection ?: Projection.getDefaultInstance()
    }
}

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50055

    val logic: ReceiptProjectorLogic = DefaultReceiptProjectorLogic()
    val service = ReceiptProjectorService(logic)
    val health = HealthStatusManager()

    val server: Server = ServerBuilder.forPort(port)
        .addService(service)
        .addService(health.healthService)
        .build()
        .start()

    health.setStatus("", HealthCheckResponse.ServingStatus.SERVING)
    logger.info("Projector server started: name={}, port={}", PROJECTOR_NAME, port)

    Runtime.getRuntime().addShutdownHook(Thread {
        server.shutdown()
    })

    server.awaitTermination()
}
