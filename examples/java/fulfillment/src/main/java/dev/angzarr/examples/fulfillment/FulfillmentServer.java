package dev.angzarr.examples.fulfillment;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Timestamp;
import examples.Domains.*;
import io.grpc.Server;
import io.grpc.ServerBuilder;
import io.grpc.Status;
import io.grpc.health.v1.HealthCheckResponse;
import io.grpc.protobuf.services.HealthStatusManager;
import io.grpc.stub.StreamObserver;
import dev.angzarr.*;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

import java.io.IOException;
import java.time.Instant;

import static net.logstash.logback.argument.StructuredArguments.kv;

public class FulfillmentServer extends BusinessLogicGrpc.BusinessLogicImplBase {
    private static final Logger logger = LoggerFactory.getLogger(FulfillmentServer.class);
    private static final String DOMAIN = "fulfillment";
    private static final int DEFAULT_PORT = 50705;

    public static void main(String[] args) throws IOException, InterruptedException {
        int port = DEFAULT_PORT;
        String portEnv = System.getenv("PORT");
        if (portEnv != null && !portEnv.isBlank()) {
            try { port = Integer.parseInt(portEnv); }
            catch (NumberFormatException e) { logger.warn("Invalid PORT: {}", portEnv); }
        }

        HealthStatusManager health = new HealthStatusManager();
        Server server = ServerBuilder.forPort(port)
            .addService(new FulfillmentServer())
            .addService(health.getHealthService())
            .build();

        server.start();
        health.setStatus("", HealthCheckResponse.ServingStatus.SERVING);
        logger.info("Business logic server started: domain={}, port={}", DOMAIN, port);

        Runtime.getRuntime().addShutdownHook(new Thread(server::shutdown));
        server.awaitTermination();
    }

    @Override
    public void handle(ContextualCommand request, StreamObserver<BusinessResponse> responseObserver) {
        try {
            EventBook events = processCommand(request);
            responseObserver.onNext(BusinessResponse.newBuilder().setEvents(events).build());
            responseObserver.onCompleted();
        } catch (CommandValidationException e) {
            responseObserver.onError(Status.fromCode(e.getStatusCode()).withDescription(e.getMessage()).asRuntimeException());
        } catch (InvalidProtocolBufferException e) {
            responseObserver.onError(Status.INVALID_ARGUMENT.withDescription("Failed to parse command: " + e.getMessage()).asRuntimeException());
        } catch (Exception e) {
            logger.error("Unexpected error", e);
            responseObserver.onError(Status.INTERNAL.withDescription("Internal error: " + e.getMessage()).asRuntimeException());
        }
    }

    private FulfillmentState rebuildState(EventBook eventBook) {
        if (eventBook == null || eventBook.getPagesList().isEmpty()) return FulfillmentState.empty();

        String orderId = "", status = "", trackingNumber = "", carrier = "";
        String pickerId = "", packerId = "", signature = "";

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) continue;
            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            try {
                if (typeUrl.endsWith("ShipmentCreated")) {
                    ShipmentCreated e = event.unpack(ShipmentCreated.class);
                    orderId = e.getOrderId();
                    status = "pending";
                } else if (typeUrl.endsWith("ItemsPicked")) {
                    ItemsPicked e = event.unpack(ItemsPicked.class);
                    status = "picking";
                    pickerId = e.getPickerId();
                } else if (typeUrl.endsWith("ItemsPacked")) {
                    ItemsPacked e = event.unpack(ItemsPacked.class);
                    status = "packing";
                    packerId = e.getPackerId();
                } else if (typeUrl.endsWith("Shipped")) {
                    Shipped e = event.unpack(Shipped.class);
                    status = "shipped";
                    trackingNumber = e.getTrackingNumber();
                    carrier = e.getCarrier();
                } else if (typeUrl.endsWith("Delivered")) {
                    Delivered e = event.unpack(Delivered.class);
                    status = "delivered";
                    signature = e.getSignature();
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack event: {}", typeUrl, e);
            }
        }

        return new FulfillmentState(orderId, status, trackingNumber, carrier, pickerId, packerId, signature);
    }

    private EventBook processCommand(ContextualCommand request) throws CommandValidationException, InvalidProtocolBufferException {
        CommandBook cmdBook = request.getCommand();
        EventBook priorEvents = request.getEvents();

        if (cmdBook == null || cmdBook.getPagesList().isEmpty())
            throw CommandValidationException.invalidArgument("CommandBook has no pages");
        var cmdPage = cmdBook.getPages(0);
        if (!cmdPage.hasCommand())
            throw CommandValidationException.invalidArgument("Command page has no command");

        FulfillmentState state = rebuildState(priorEvents);
        Any command = cmdPage.getCommand();
        String typeUrl = command.getTypeUrl();
        int seq = priorEvents != null ? priorEvents.getPagesCount() : 0;

        EventBook.Builder result = EventBook.newBuilder().setCover(cmdBook.getCover());

        if (typeUrl.endsWith("CreateShipment")) {
            handleCreateShipment(state, command.unpack(CreateShipment.class), result, seq);
        } else if (typeUrl.endsWith("MarkPicked")) {
            handleMarkPicked(state, command.unpack(MarkPicked.class), result, seq);
        } else if (typeUrl.endsWith("MarkPacked")) {
            handleMarkPacked(state, command.unpack(MarkPacked.class), result, seq);
        } else if (typeUrl.endsWith("Ship")) {
            handleShip(state, command.unpack(Ship.class), result, seq);
        } else if (typeUrl.endsWith("RecordDelivery")) {
            handleRecordDelivery(state, command.unpack(RecordDelivery.class), result, seq);
        } else {
            throw CommandValidationException.invalidArgument("Unknown command type: " + typeUrl);
        }

        return result.build();
    }

    private void handleCreateShipment(FulfillmentState state, CreateShipment cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (state.exists()) throw CommandValidationException.failedPrecondition("Shipment already exists");
        if (cmd.getOrderId().isBlank()) throw CommandValidationException.invalidArgument("Order ID is required");

        logger.info("creating_shipment", kv("order_id", cmd.getOrderId()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(ShipmentCreated.newBuilder()
                .setOrderId(cmd.getOrderId())
                .setCreatedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleMarkPicked(FulfillmentState state, MarkPicked cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Shipment does not exist");
        if (!"pending".equals(state.status())) throw CommandValidationException.failedPrecondition("Cannot pick from status: " + state.status());
        if (cmd.getPickerId().isBlank()) throw CommandValidationException.invalidArgument("Picker ID is required");

        logger.info("marking_picked", kv("order_id", state.orderId()), kv("picker_id", cmd.getPickerId()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(ItemsPicked.newBuilder()
                .setPickerId(cmd.getPickerId())
                .setPickedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleMarkPacked(FulfillmentState state, MarkPacked cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Shipment does not exist");
        if (!"picking".equals(state.status())) throw CommandValidationException.failedPrecondition("Cannot pack from status: " + state.status());
        if (cmd.getPackerId().isBlank()) throw CommandValidationException.invalidArgument("Packer ID is required");

        logger.info("marking_packed", kv("order_id", state.orderId()), kv("packer_id", cmd.getPackerId()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(ItemsPacked.newBuilder()
                .setPackerId(cmd.getPackerId())
                .setPackedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleShip(FulfillmentState state, Ship cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Shipment does not exist");
        if (!"packing".equals(state.status())) throw CommandValidationException.failedPrecondition("Cannot ship from status: " + state.status());
        if (cmd.getTrackingNumber().isBlank()) throw CommandValidationException.invalidArgument("Tracking number is required");
        if (cmd.getCarrier().isBlank()) throw CommandValidationException.invalidArgument("Carrier is required");

        logger.info("shipping", kv("order_id", state.orderId()), kv("carrier", cmd.getCarrier()), kv("tracking", cmd.getTrackingNumber()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(Shipped.newBuilder()
                .setTrackingNumber(cmd.getTrackingNumber())
                .setCarrier(cmd.getCarrier())
                .setShippedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleRecordDelivery(FulfillmentState state, RecordDelivery cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Shipment does not exist");
        if (!"shipped".equals(state.status())) throw CommandValidationException.failedPrecondition("Cannot record delivery from status: " + state.status());

        logger.info("recording_delivery", kv("order_id", state.orderId()), kv("signature", cmd.getSignature()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(Delivered.newBuilder()
                .setDeliveredAt(nowTimestamp())
                .setSignature(cmd.getSignature())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private Timestamp nowTimestamp() {
        Instant now = Instant.now();
        return Timestamp.newBuilder().setSeconds(now.getEpochSecond()).setNanos(now.getNano()).build();
    }
}
