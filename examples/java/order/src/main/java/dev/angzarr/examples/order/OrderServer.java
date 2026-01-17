package dev.angzarr.examples.order;

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
import java.util.ArrayList;
import java.util.stream.Collectors;

import static net.logstash.logback.argument.StructuredArguments.kv;

public class OrderServer extends BusinessLogicGrpc.BusinessLogicImplBase {
    private static final Logger logger = LoggerFactory.getLogger(OrderServer.class);
    private static final String DOMAIN = "order";
    private static final int DEFAULT_PORT = 50703;

    public static void main(String[] args) throws IOException, InterruptedException {
        int port = DEFAULT_PORT;
        String portEnv = System.getenv("PORT");
        if (portEnv != null && !portEnv.isBlank()) {
            try { port = Integer.parseInt(portEnv); }
            catch (NumberFormatException e) { logger.warn("Invalid PORT: {}", portEnv); }
        }

        HealthStatusManager health = new HealthStatusManager();
        Server server = ServerBuilder.forPort(port)
            .addService(new OrderServer())
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

    private OrderState rebuildState(EventBook eventBook) {
        if (eventBook == null || eventBook.getPagesList().isEmpty()) return OrderState.empty();

        String customerId = "";
        var items = new ArrayList<OrderState.LineItem>();
        int subtotalCents = 0, discountCents = 0, loyaltyPointsUsed = 0;
        String paymentMethod = "", paymentReference = "", status = "";

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) continue;
            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            try {
                if (typeUrl.endsWith("OrderCreated")) {
                    OrderCreated e = event.unpack(OrderCreated.class);
                    customerId = e.getCustomerId();
                    items = e.getItemsList().stream()
                        .map(i -> new OrderState.LineItem(i.getProductId(), i.getName(), i.getQuantity(), i.getUnitPriceCents()))
                        .collect(Collectors.toCollection(ArrayList::new));
                    subtotalCents = e.getSubtotalCents();
                    discountCents = e.getDiscountCents();
                    status = "pending";
                } else if (typeUrl.endsWith("LoyaltyDiscountApplied")) {
                    LoyaltyDiscountApplied e = event.unpack(LoyaltyDiscountApplied.class);
                    loyaltyPointsUsed = e.getPointsUsed();
                    discountCents += e.getDiscountCents();
                } else if (typeUrl.endsWith("PaymentSubmitted")) {
                    PaymentSubmitted e = event.unpack(PaymentSubmitted.class);
                    paymentMethod = e.getPaymentMethod();
                    paymentReference = e.getPaymentReference();
                    status = "payment_submitted";
                } else if (typeUrl.endsWith("PaymentConfirmed")) {
                    status = "payment_confirmed";
                } else if (typeUrl.endsWith("OrderCompleted")) {
                    status = "completed";
                } else if (typeUrl.endsWith("OrderCancelled")) {
                    status = "cancelled";
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack event: {}", typeUrl, e);
            }
        }

        return new OrderState(customerId, items, subtotalCents, discountCents, loyaltyPointsUsed, paymentMethod, paymentReference, status);
    }

    private EventBook processCommand(ContextualCommand request) throws CommandValidationException, InvalidProtocolBufferException {
        CommandBook cmdBook = request.getCommand();
        EventBook priorEvents = request.getEvents();

        if (cmdBook == null || cmdBook.getPagesList().isEmpty())
            throw CommandValidationException.invalidArgument("CommandBook has no pages");
        var cmdPage = cmdBook.getPages(0);
        if (!cmdPage.hasCommand())
            throw CommandValidationException.invalidArgument("Command page has no command");

        OrderState state = rebuildState(priorEvents);
        Any command = cmdPage.getCommand();
        String typeUrl = command.getTypeUrl();
        int seq = priorEvents != null ? priorEvents.getPagesCount() : 0;

        EventBook.Builder result = EventBook.newBuilder().setCover(cmdBook.getCover());

        if (typeUrl.endsWith("CreateOrder")) {
            handleCreateOrder(state, command.unpack(CreateOrder.class), result, seq);
        } else if (typeUrl.endsWith("ApplyLoyaltyDiscount")) {
            handleApplyLoyaltyDiscount(state, command.unpack(ApplyLoyaltyDiscount.class), result, seq);
        } else if (typeUrl.endsWith("SubmitPayment")) {
            handleSubmitPayment(state, command.unpack(SubmitPayment.class), result, seq);
        } else if (typeUrl.endsWith("ConfirmPayment")) {
            handleConfirmPayment(state, result, seq);
        } else if (typeUrl.endsWith("CancelOrder")) {
            handleCancelOrder(state, command.unpack(CancelOrder.class), result, seq);
        } else {
            throw CommandValidationException.invalidArgument("Unknown command type: " + typeUrl);
        }

        return result.build();
    }

    private void handleCreateOrder(OrderState state, CreateOrder cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (state.exists()) throw CommandValidationException.failedPrecondition("Order already exists");
        if (cmd.getCustomerId().isBlank()) throw CommandValidationException.invalidArgument("Customer ID is required");
        if (cmd.getItemsList().isEmpty()) throw CommandValidationException.invalidArgument("Order must have at least one item");

        logger.info("creating_order", kv("customer_id", cmd.getCustomerId()), kv("item_count", cmd.getItemsCount()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(OrderCreated.newBuilder()
                .setCustomerId(cmd.getCustomerId())
                .addAllItems(cmd.getItemsList())
                .setSubtotalCents(cmd.getSubtotalCents())
                .setDiscountCents(cmd.getDiscountCents())
                .setCreatedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleApplyLoyaltyDiscount(OrderState state, ApplyLoyaltyDiscount cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Order does not exist");
        if (!state.isPending()) throw CommandValidationException.failedPrecondition("Cannot apply discount to non-pending order");
        if (state.loyaltyPointsUsed() > 0) throw CommandValidationException.failedPrecondition("Loyalty discount already applied");
        if (cmd.getPoints() <= 0) throw CommandValidationException.invalidArgument("Points must be positive");

        int discountCents = cmd.getPoints();
        logger.info("applying_loyalty_discount", kv("points", cmd.getPoints()), kv("discount", discountCents));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(LoyaltyDiscountApplied.newBuilder()
                .setPointsUsed(cmd.getPoints())
                .setDiscountCents(discountCents)
                .setAppliedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleSubmitPayment(OrderState state, SubmitPayment cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Order does not exist");
        if (!state.isPending()) throw CommandValidationException.failedPrecondition("Payment already submitted or order not pending");
        if (cmd.getPaymentMethod().isBlank()) throw CommandValidationException.invalidArgument("Payment method is required");

        int totalCents = state.totalCents();
        logger.info("submitting_payment", kv("payment_method", cmd.getPaymentMethod()), kv("total", totalCents));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(PaymentSubmitted.newBuilder()
                .setPaymentMethod(cmd.getPaymentMethod())
                .setPaymentReference(cmd.getPaymentReference())
                .setAmountCents(totalCents)
                .setSubmittedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleConfirmPayment(OrderState state, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Order does not exist");
        if (!"payment_submitted".equals(state.status())) throw CommandValidationException.failedPrecondition("Payment not submitted");

        logger.info("confirming_payment", kv("customer_id", state.customerId()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(PaymentConfirmed.newBuilder().setConfirmedAt(nowTimestamp()).build()))
            .setCreatedAt(nowTimestamp())
            .build());

        result.addPages(EventPage.newBuilder()
            .setNum(seq + 1)
            .setEvent(Any.pack(OrderCompleted.newBuilder().setCompletedAt(nowTimestamp()).build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleCancelOrder(OrderState state, CancelOrder cmd, EventBook.Builder result, int seq) throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Order does not exist");
        if ("cancelled".equals(state.status())) throw CommandValidationException.failedPrecondition("Order already cancelled");
        if ("completed".equals(state.status())) throw CommandValidationException.failedPrecondition("Cannot cancel completed order");

        logger.info("cancelling_order", kv("customer_id", state.customerId()), kv("reason", cmd.getReason()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(OrderCancelled.newBuilder()
                .setReason(cmd.getReason())
                .setLoyaltyPointsUsed(state.loyaltyPointsUsed())
                .setCancelledAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private Timestamp nowTimestamp() {
        Instant now = Instant.now();
        return Timestamp.newBuilder().setSeconds(now.getEpochSecond()).setNanos(now.getNano()).build();
    }
}
