package dev.angzarr.examples.inventory;

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
import java.util.HashMap;

import static net.logstash.logback.argument.StructuredArguments.kv;

public class InventoryServer extends BusinessLogicGrpc.BusinessLogicImplBase {
    private static final Logger logger = LoggerFactory.getLogger(InventoryServer.class);
    private static final String DOMAIN = "inventory";
    private static final int DEFAULT_PORT = 50704;

    public static void main(String[] args) throws IOException, InterruptedException {
        int port = DEFAULT_PORT;
        String portEnv = System.getenv("PORT");
        if (portEnv != null && !portEnv.isBlank()) {
            try { port = Integer.parseInt(portEnv); }
            catch (NumberFormatException e) { logger.warn("Invalid PORT: {}", portEnv); }
        }

        HealthStatusManager health = new HealthStatusManager();
        Server server = ServerBuilder.forPort(port)
            .addService(new InventoryServer())
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

    private InventoryState rebuildState(EventBook eventBook) {
        if (eventBook == null || eventBook.getPagesList().isEmpty()) return InventoryState.empty();

        String productId = "";
        int onHand = 0, reserved = 0, lowStockThreshold = 0;
        var reservations = new HashMap<String, Integer>();

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) continue;
            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            try {
                if (typeUrl.endsWith("StockInitialized")) {
                    StockInitialized e = event.unpack(StockInitialized.class);
                    productId = e.getProductId();
                    onHand = e.getInitialQuantity();
                    lowStockThreshold = e.getLowStockThreshold();
                } else if (typeUrl.endsWith("StockReceived")) {
                    StockReceived e = event.unpack(StockReceived.class);
                    onHand += e.getQuantity();
                } else if (typeUrl.endsWith("StockReserved")) {
                    StockReserved e = event.unpack(StockReserved.class);
                    reserved += e.getQuantity();
                    reservations.put(e.getOrderId(), e.getQuantity());
                } else if (typeUrl.endsWith("ReservationReleased")) {
                    ReservationReleased e = event.unpack(ReservationReleased.class);
                    int qty = reservations.getOrDefault(e.getOrderId(), 0);
                    reserved -= qty;
                    reservations.remove(e.getOrderId());
                } else if (typeUrl.endsWith("ReservationCommitted")) {
                    ReservationCommitted e = event.unpack(ReservationCommitted.class);
                    int qty = reservations.getOrDefault(e.getOrderId(), 0);
                    onHand -= qty;
                    reserved -= qty;
                    reservations.remove(e.getOrderId());
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack event: {}", typeUrl, e);
            }
        }

        return new InventoryState(productId, onHand, reserved, lowStockThreshold, reservations);
    }

    private EventBook processCommand(ContextualCommand request) throws CommandValidationException, InvalidProtocolBufferException {
        CommandBook cmdBook = request.getCommand();
        EventBook priorEvents = request.getEvents();

        if (cmdBook == null || cmdBook.getPagesList().isEmpty())
            throw CommandValidationException.invalidArgument("CommandBook has no pages");
        var cmdPage = cmdBook.getPages(0);
        if (!cmdPage.hasCommand())
            throw CommandValidationException.invalidArgument("Command page has no command");

        InventoryState state = rebuildState(priorEvents);
        Any command = cmdPage.getCommand();
        String typeUrl = command.getTypeUrl();
        int seq = priorEvents != null ? priorEvents.getPagesCount() : 0;

        EventBook.Builder result = EventBook.newBuilder().setCover(cmdBook.getCover());

        if (typeUrl.endsWith("InitializeStock")) {
            handleInitializeStock(state, command.unpack(InitializeStock.class), result, seq);
        } else if (typeUrl.endsWith("ReceiveStock")) {
            handleReceiveStock(state, command.unpack(ReceiveStock.class), result, seq);
        } else if (typeUrl.endsWith("ReserveStock")) {
            handleReserveStock(state, command.unpack(ReserveStock.class), result, seq);
        } else if (typeUrl.endsWith("ReleaseReservation")) {
            handleReleaseReservation(state, command.unpack(ReleaseReservation.class), result, seq);
        } else if (typeUrl.endsWith("CommitReservation")) {
            handleCommitReservation(state, command.unpack(CommitReservation.class), result, seq);
        } else {
            throw CommandValidationException.invalidArgument("Unknown command type: " + typeUrl);
        }

        return result.build();
    }

    private void handleInitializeStock(InventoryState state, InitializeStock cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (state.exists()) throw CommandValidationException.failedPrecondition("Inventory already initialized");
        if (cmd.getProductId().isBlank()) throw CommandValidationException.invalidArgument("Product ID is required");
        if (cmd.getInitialQuantity() < 0) throw CommandValidationException.invalidArgument("Initial quantity cannot be negative");

        logger.info("initializing_stock", kv("product_id", cmd.getProductId()), kv("quantity", cmd.getInitialQuantity()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(StockInitialized.newBuilder()
                .setProductId(cmd.getProductId())
                .setInitialQuantity(cmd.getInitialQuantity())
                .setLowStockThreshold(cmd.getLowStockThreshold() > 0 ? cmd.getLowStockThreshold() : 10)
                .setInitializedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleReceiveStock(InventoryState state, ReceiveStock cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Inventory not initialized");
        if (cmd.getQuantity() <= 0) throw CommandValidationException.invalidArgument("Quantity must be positive");

        logger.info("receiving_stock", kv("product_id", state.productId()), kv("quantity", cmd.getQuantity()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(StockReceived.newBuilder()
                .setQuantity(cmd.getQuantity())
                .setNewOnHand(state.onHand() + cmd.getQuantity())
                .setReceivedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleReserveStock(InventoryState state, ReserveStock cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Inventory not initialized");
        if (cmd.getOrderId().isBlank()) throw CommandValidationException.invalidArgument("Order ID is required");
        if (cmd.getQuantity() <= 0) throw CommandValidationException.invalidArgument("Quantity must be positive");
        if (state.reservations().containsKey(cmd.getOrderId()))
            throw CommandValidationException.failedPrecondition("Reservation already exists for order");
        if (cmd.getQuantity() > state.available())
            throw CommandValidationException.failedPrecondition(
                String.format("Insufficient stock: available %d, requested %d", state.available(), cmd.getQuantity()));

        logger.info("reserving_stock", kv("product_id", state.productId()),
            kv("order_id", cmd.getOrderId()), kv("quantity", cmd.getQuantity()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(StockReserved.newBuilder()
                .setOrderId(cmd.getOrderId())
                .setQuantity(cmd.getQuantity())
                .setReservedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());

        int newAvailable = state.available() - cmd.getQuantity();
        if (newAvailable <= state.lowStockThreshold()) {
            result.addPages(EventPage.newBuilder()
                .setNum(seq + 1)
                .setEvent(Any.pack(LowStockAlert.newBuilder()
                    .setAvailableQuantity(newAvailable)
                    .setThreshold(state.lowStockThreshold())
                    .setAlertedAt(nowTimestamp())
                    .build()))
                .setCreatedAt(nowTimestamp())
                .build());
        }
    }

    private void handleReleaseReservation(InventoryState state, ReleaseReservation cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Inventory not initialized");
        if (cmd.getOrderId().isBlank()) throw CommandValidationException.invalidArgument("Order ID is required");
        if (!state.reservations().containsKey(cmd.getOrderId()))
            throw CommandValidationException.failedPrecondition("Reservation not found");

        int releasedQty = state.reservations().get(cmd.getOrderId());
        logger.info("releasing_reservation", kv("product_id", state.productId()),
            kv("order_id", cmd.getOrderId()), kv("quantity", releasedQty));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(ReservationReleased.newBuilder()
                .setOrderId(cmd.getOrderId())
                .setReleasedQuantity(releasedQty)
                .setReleasedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleCommitReservation(InventoryState state, CommitReservation cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Inventory not initialized");
        if (cmd.getOrderId().isBlank()) throw CommandValidationException.invalidArgument("Order ID is required");
        if (!state.reservations().containsKey(cmd.getOrderId()))
            throw CommandValidationException.failedPrecondition("Reservation not found");

        int committedQty = state.reservations().get(cmd.getOrderId());
        logger.info("committing_reservation", kv("product_id", state.productId()),
            kv("order_id", cmd.getOrderId()), kv("quantity", committedQty));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(ReservationCommitted.newBuilder()
                .setOrderId(cmd.getOrderId())
                .setCommittedQuantity(committedQty)
                .setCommittedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private Timestamp nowTimestamp() {
        Instant now = Instant.now();
        return Timestamp.newBuilder().setSeconds(now.getEpochSecond()).setNanos(now.getNano()).build();
    }
}
