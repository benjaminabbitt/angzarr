/**
 * Transaction Log Projector - Kotlin Implementation
 */
package dev.angzarr.examples.projector

import dev.angzarr.*
import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.protobuf.services.HealthStatusManager
import org.slf4j.LoggerFactory

private val logger = LoggerFactory.getLogger("TransactionLogProjector")
private const val PROJECTOR_NAME = "log-transaction"

/**
 * gRPC service adapter for transaction log projector.
 */
class TransactionLogProjectorService(
    private val logic: LogProjectorLogic
) : ProjectorCoordinatorGrpcKt.ProjectorCoordinatorCoroutineImplBase() {

    override suspend fun handle(request: EventBook): com.google.protobuf.Empty {
        logEvents(request)
        return com.google.protobuf.Empty.getDefaultInstance()
    }

    override suspend fun handleSync(request: EventBook): Projection {
        logEvents(request)
        return Projection.getDefaultInstance()
    }

    private fun logEvents(eventBook: EventBook) {
        val entries = logic.processEvents(eventBook)
        for (entry in entries) {
            val shortId = entry.rootId.take(16)
            val fieldsStr = entry.fields.entries.joinToString(" ") { "${it.key}=${it.value}" }
            logger.info(
                "event domain={} root_id={} seq={} type={} {}",
                entry.domain, shortId, entry.sequence, entry.eventType, fieldsStr
            )
        }
    }
}

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50057

    val logic: LogProjectorLogic = DefaultTransactionLogProjectorLogic()
    val service = TransactionLogProjectorService(logic)
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
