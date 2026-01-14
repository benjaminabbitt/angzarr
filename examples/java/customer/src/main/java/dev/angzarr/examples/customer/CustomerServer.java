package dev.angzarr.examples.customer;

import io.grpc.Server;
import io.grpc.ServerBuilder;
import io.grpc.health.v1.HealthCheckResponse;
import io.grpc.protobuf.services.HealthStatusManager;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;

/**
 * Main server entry point for the customer business logic service.
 */
public class CustomerServer {
    private static final Logger logger = LoggerFactory.getLogger(CustomerServer.class);
    private static final String DOMAIN = "customer";
    private static final int DEFAULT_PORT = 50052;

    private final Server server;
    private final HealthStatusManager health = new HealthStatusManager();

    public CustomerServer(int port, CustomerLogic logic) {
        this.server = ServerBuilder.forPort(port)
            .addService(new CustomerService(logic))
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
            CustomerServer.this.stop();
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

        // Dependency injection: create the business logic implementation
        CustomerLogic logic = new DefaultCustomerLogic();

        CustomerServer server = new CustomerServer(port, logic);
        server.start();
        server.blockUntilShutdown();
    }
}
