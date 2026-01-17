package dev.angzarr.examples.saga.loyaltyearn;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Empty;
import examples.Domains.*;
import io.grpc.Server;
import io.grpc.ServerBuilder;
import io.grpc.health.v1.HealthCheckResponse;
import io.grpc.protobuf.services.HealthStatusManager;
import io.grpc.stub.StreamObserver;
import dev.angzarr.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

import static net.logstash.logback.argument.StructuredArguments.kv;

public class LoyaltyEarnSagaServer extends SagaGrpc.SagaImplBase {
    private static final Logger logger = LoggerFactory.getLogger(LoyaltyEarnSagaServer.class);
    private static final String SAGA_NAME = "loyalty-earn";
    private static final String SOURCE_DOMAIN = "fulfillment";
    private static final int DEFAULT_PORT = 50708;
    private static final int POINTS_PER_DOLLAR = 10;

    public static void main(String[] args) throws IOException, InterruptedException {
        int port = DEFAULT_PORT;
        String portEnv = System.getenv("PORT");
        if (portEnv != null && !portEnv.isBlank()) {
            try { port = Integer.parseInt(portEnv); }
            catch (NumberFormatException e) { logger.warn("Invalid PORT: {}", portEnv); }
        }

        HealthStatusManager health = new HealthStatusManager();
        Server server = ServerBuilder.forPort(port)
            .addService(new LoyaltyEarnSagaServer())
            .addService(health.getHealthService())
            .build();

        server.start();
        health.setStatus("", HealthCheckResponse.ServingStatus.SERVING);
        logger.info("Saga server started: saga={}, port={}, source_domain={}", SAGA_NAME, port, SOURCE_DOMAIN);

        Runtime.getRuntime().addShutdownHook(new Thread(server::shutdown));
        server.awaitTermination();
    }

    @Override
    public void handle(EventBook request, StreamObserver<Empty> responseObserver) {
        List<CommandBook> commands = processEvents(request);
        if (!commands.isEmpty()) {
            logger.info("processed_delivery_for_loyalty", kv("command_count", commands.size()));
        }
        responseObserver.onNext(Empty.getDefaultInstance());
        responseObserver.onCompleted();
    }

    @Override
    public void handleSync(EventBook request, StreamObserver<SagaResponse> responseObserver) {
        List<CommandBook> commands = processEvents(request);
        responseObserver.onNext(SagaResponse.newBuilder().addAllCommands(commands).build());
        responseObserver.onCompleted();
    }

    private List<CommandBook> processEvents(EventBook eventBook) {
        List<CommandBook> commands = new ArrayList<>();

        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return commands;
        }

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) continue;
            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            if (!typeUrl.endsWith("Delivered")) continue;

            String orderId = "";
            if (eventBook.hasCover() && eventBook.getCover().hasRoot()) {
                ByteString rootBytes = eventBook.getCover().getRoot().getValue();
                orderId = bytesToHex(rootBytes.toByteArray());
            }

            if (orderId.isEmpty()) continue;

            int points = POINTS_PER_DOLLAR * 100;

            logger.info("awarding_loyalty_points", kv("order_id", orderId), kv("points", points));

            AddLoyaltyPoints addPointsCmd = AddLoyaltyPoints.newBuilder()
                .setPoints(points)
                .setReason("Order delivery: " + orderId)
                .build();

            CommandBook cmdBook = CommandBook.newBuilder()
                .setCover(Cover.newBuilder()
                    .setDomain("customer")
                    .build())
                .addPages(CommandPage.newBuilder()
                    .setSequence(0)
                    .setSynchronous(false)
                    .setCommand(Any.pack(addPointsCmd))
                    .build())
                .setCorrelationId(eventBook.getCorrelationId())
                .build();

            commands.add(cmdBook);
        }

        return commands;
    }

    private static String bytesToHex(byte[] bytes) {
        StringBuilder sb = new StringBuilder();
        for (byte b : bytes) {
            sb.append(String.format("%02x", b));
        }
        return sb.toString();
    }
}
