package dev.angzarr.examples.saga.cancellation;

import com.google.protobuf.Any;
import com.google.protobuf.ByteString;
import com.google.protobuf.Empty;
import com.google.protobuf.InvalidProtocolBufferException;
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

public class CancellationSagaServer extends SagaGrpc.SagaImplBase {
    private static final Logger logger = LoggerFactory.getLogger(CancellationSagaServer.class);
    private static final String SAGA_NAME = "cancellation";
    private static final String SOURCE_DOMAIN = "order";
    private static final int DEFAULT_PORT = 50709;

    public static void main(String[] args) throws IOException, InterruptedException {
        int port = DEFAULT_PORT;
        String portEnv = System.getenv("PORT");
        if (portEnv != null && !portEnv.isBlank()) {
            try { port = Integer.parseInt(portEnv); }
            catch (NumberFormatException e) { logger.warn("Invalid PORT: {}", portEnv); }
        }

        HealthStatusManager health = new HealthStatusManager();
        Server server = ServerBuilder.forPort(port)
            .addService(new CancellationSagaServer())
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
            logger.info("processed_cancellation", kv("compensation_commands", commands.size()));
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

            if (!typeUrl.endsWith("OrderCancelled")) continue;

            try {
                OrderCancelled cancelledEvent = event.unpack(OrderCancelled.class);

                String orderId = "";
                if (eventBook.hasCover() && eventBook.getCover().hasRoot()) {
                    ByteString rootBytes = eventBook.getCover().getRoot().getValue();
                    orderId = bytesToHex(rootBytes.toByteArray());
                }

                if (orderId.isEmpty()) continue;

                logger.info("processing_order_cancellation", kv("order_id", orderId));

                ReleaseReservation releaseCmd = ReleaseReservation.newBuilder()
                    .setOrderId(orderId)
                    .build();

                CommandBook releaseCmdBook = CommandBook.newBuilder()
                    .setCover(Cover.newBuilder()
                        .setDomain("inventory")
                        .setRoot(eventBook.getCover().getRoot())
                        .build())
                    .addPages(CommandPage.newBuilder()
                        .setSequence(0)
                        .setSynchronous(false)
                        .setCommand(Any.pack(releaseCmd))
                        .build())
                    .setCorrelationId(eventBook.getCorrelationId())
                    .build();

                commands.add(releaseCmdBook);

                int loyaltyPointsUsed = cancelledEvent.getLoyaltyPointsUsed();
                if (loyaltyPointsUsed > 0) {
                    AddLoyaltyPoints addPointsCmd = AddLoyaltyPoints.newBuilder()
                        .setPoints(loyaltyPointsUsed)
                        .setReason("Order cancellation refund")
                        .build();

                    CommandBook addPointsCmdBook = CommandBook.newBuilder()
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

                    commands.add(addPointsCmdBook);
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack OrderCancelled event", e);
            }
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
