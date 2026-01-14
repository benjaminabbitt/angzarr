/**
 * Transaction Service - Kotlin Implementation
 *
 * Entry point for the transaction business logic gRPC server.
 */
package dev.angzarr.examples.transaction

import io.grpc.Server
import io.grpc.ServerBuilder
import io.grpc.health.v1.HealthCheckResponse
import io.grpc.protobuf.services.HealthStatusManager
import org.slf4j.LoggerFactory

private val logger = LoggerFactory.getLogger("TransactionServer")
private const val DOMAIN = "transaction"

fun main() {
    val port = System.getenv("PORT")?.toIntOrNull() ?: 50053

    val logic: TransactionLogic = DefaultTransactionLogic()
    val service = TransactionService(logic)
    val health = HealthStatusManager()

    val server: Server = ServerBuilder.forPort(port)
        .addService(service)
        .addService(health.healthService)
        .build()
        .start()

    health.setStatus("", HealthCheckResponse.ServingStatus.SERVING)
    logger.info("Business logic server started: domain={}, port={}", DOMAIN, port)

    Runtime.getRuntime().addShutdownHook(Thread {
        server.shutdown()
    })

    server.awaitTermination()
}
