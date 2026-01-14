package dev.angzarr.examples.projector;

import io.grpc.Server;
import io.grpc.ServerBuilder;
import io.grpc.health.v1.HealthCheckResponse;
import io.grpc.protobuf.services.HealthStatusManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

/**
 * Main server for customer log projector.
 */
public class CustomerLogProjectorServer {
    private static final Logger logger = LoggerFactory.getLogger(CustomerLogProjectorServer.class);
    private static final String PROJECTOR_NAME = "log-customer";
    private static final int DEFAULT_PORT = 50056;

    private final Server server;
    private final HealthStatusManager health = new HealthStatusManager();

    public CustomerLogProjectorServer(int port, CustomerLogProjector projector) {
        this.server = ServerBuilder.forPort(port)
            .addService(new CustomerLogProjectorService(projector))
            .addService(health.getHealthService())
            .build();
    }

    public void start() throws IOException {
        server.start();
        health.setStatus("", HealthCheckResponse.ServingStatus.SERVING);
        logger.info("Projector server started: name={}, port={}, listens_to=customer domain",
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

        CustomerLogProjector projector = new DefaultCustomerLogProjector();
        CustomerLogProjectorServer server = new CustomerLogProjectorServer(port, projector);
        server.start();
        server.blockUntilShutdown();
    }
}
