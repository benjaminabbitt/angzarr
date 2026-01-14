package dev.angzarr.examples.projector;

import io.grpc.Server;
import io.grpc.ServerBuilder;
import io.grpc.health.v1.HealthCheckResponse;
import io.grpc.protobuf.services.HealthStatusManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

/**
 * Main server for receipt projector.
 */
public class ReceiptProjectorServer {
    private static final Logger logger = LoggerFactory.getLogger(ReceiptProjectorServer.class);
    private static final String PROJECTOR_NAME = "receipt";
    private static final int DEFAULT_PORT = 50055;

    private final Server server;
    private final HealthStatusManager health = new HealthStatusManager();

    public ReceiptProjectorServer(int port, ReceiptProjector projector) {
        this.server = ServerBuilder.forPort(port)
            .addService(new ReceiptProjectorService(projector))
            .addService(health.getHealthService())
            .build();
    }

    public void start() throws IOException {
        server.start();
        health.setStatus("", HealthCheckResponse.ServingStatus.SERVING);
        logger.info("Projector server started: name={}, port={}, listens_to=transaction domain",
            PROJECTOR_NAME, server.getPort());

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

        ReceiptProjector projector = new DefaultReceiptProjector();
        ReceiptProjectorServer server = new ReceiptProjectorServer(port, projector);
        server.start();
        server.blockUntilShutdown();
    }
}
