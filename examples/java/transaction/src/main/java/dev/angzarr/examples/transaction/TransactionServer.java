package dev.angzarr.examples.transaction;

import io.grpc.Server;
import io.grpc.ServerBuilder;
import io.grpc.health.v1.HealthCheckResponse;
import io.grpc.protobuf.services.HealthStatusManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

/**
 * Main server entry point for the transaction business logic service.
 */
public class TransactionServer {
    private static final Logger logger = LoggerFactory.getLogger(TransactionServer.class);
    private static final String DOMAIN = "transaction";
    private static final int DEFAULT_PORT = 50053;

    private final Server server;
    private final HealthStatusManager health = new HealthStatusManager();

    public TransactionServer(int port, TransactionLogic logic) {
        this.server = ServerBuilder.forPort(port)
            .addService(new TransactionService(logic))
            .addService(health.getHealthService())
            .build();
    }

    public void start() throws IOException {
        server.start();
        health.setStatus("", HealthCheckResponse.ServingStatus.SERVING);
        logger.info("Business logic server started: domain={}, port={}",
            DOMAIN, server.getPort());

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            logger.info("Shutting down server...");
            TransactionServer.this.stop();
        }));
    }

    public void stop() {
        if (server != null) {
            server.shutdown();
        }
    }

    public void blockUntilShutdown() throws InterruptedException {
        if (server != null) {
            server.awaitTermination();
        }
    }

    public static void main(String[] args) throws IOException, InterruptedException {
        int port = DEFAULT_PORT;
        String portEnv = System.getenv("PORT");
        if (portEnv != null && !portEnv.isBlank()) {
            try {
                port = Integer.parseInt(portEnv);
            } catch (NumberFormatException e) {
                logger.warn("Invalid PORT env var '{}', using default {}", portEnv, DEFAULT_PORT);
            }
        }

        TransactionLogic logic = new DefaultTransactionLogic();
        TransactionServer server = new TransactionServer(port, logic);
        server.start();
        server.blockUntilShutdown();
    }
}
