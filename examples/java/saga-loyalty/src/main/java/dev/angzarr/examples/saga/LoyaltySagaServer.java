package dev.angzarr.examples.saga;

import io.grpc.Server;
import io.grpc.ServerBuilder;
import io.grpc.health.v1.HealthCheckResponse;
import io.grpc.protobuf.services.HealthStatusManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

/**
 * Main server for loyalty points saga.
 */
public class LoyaltySagaServer {
    private static final Logger logger = LoggerFactory.getLogger(LoyaltySagaServer.class);
    private static final String SAGA_NAME = "loyalty_points";
    private static final int DEFAULT_PORT = 50054;

    private final Server server;
    private final HealthStatusManager health = new HealthStatusManager();

    public LoyaltySagaServer(int port, LoyaltySaga saga) {
        this.server = ServerBuilder.forPort(port)
            .addService(new LoyaltySagaService(saga))
            .addService(health.getHealthService())
            .build();
    }

    public void start() throws IOException {
        server.start();
        health.setStatus("", HealthCheckResponse.ServingStatus.SERVING);
        logger.info("Saga server started: name={}, port={}, listens_to=transaction domain",
            SAGA_NAME, server.getPort());

        Runtime.getRuntime().addShutdownHook(new Thread(this::stop));
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
                logger.warn("Invalid PORT, using default {}", DEFAULT_PORT);
            }
        }

        LoyaltySaga saga = new DefaultLoyaltySaga();
        LoyaltySagaServer server = new LoyaltySagaServer(port, saga);
        server.start();
        server.blockUntilShutdown();
    }
}
