package dev.angzarr.examples.cart;

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
import java.util.stream.Collectors;

import static net.logstash.logback.argument.StructuredArguments.kv;

public class CartServer extends BusinessLogicGrpc.BusinessLogicImplBase {
    private static final Logger logger = LoggerFactory.getLogger(CartServer.class);
    private static final String DOMAIN = "cart";
    private static final int DEFAULT_PORT = 50702;

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

        HealthStatusManager health = new HealthStatusManager();
        Server server = ServerBuilder.forPort(port)
            .addService(new CartServer())
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
            responseObserver.onError(Status.fromCode(e.getStatusCode())
                .withDescription(e.getMessage()).asRuntimeException());
        } catch (InvalidProtocolBufferException e) {
            responseObserver.onError(Status.INVALID_ARGUMENT
                .withDescription("Failed to parse command: " + e.getMessage()).asRuntimeException());
        } catch (Exception e) {
            logger.error("Unexpected error", e);
            responseObserver.onError(Status.INTERNAL
                .withDescription("Internal error: " + e.getMessage()).asRuntimeException());
        }
    }

    private CartState rebuildState(EventBook eventBook) {
        var items = new HashMap<String, CartState.CartItem>();
        String customerId = "";
        String couponCode = "";
        int discountCents = 0;
        String status = "";

        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return CartState.empty();
        }

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) continue;
            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            try {
                if (typeUrl.endsWith("CartCreated")) {
                    CartCreated e = event.unpack(CartCreated.class);
                    customerId = e.getCustomerId();
                    status = "active";
                } else if (typeUrl.endsWith("ItemAdded")) {
                    ItemAdded e = event.unpack(ItemAdded.class);
                    items.put(e.getProductId(), new CartState.CartItem(
                        e.getProductId(), e.getName(), e.getQuantity(), e.getUnitPriceCents()));
                } else if (typeUrl.endsWith("QuantityUpdated")) {
                    QuantityUpdated e = event.unpack(QuantityUpdated.class);
                    var item = items.get(e.getProductId());
                    if (item != null) {
                        items.put(e.getProductId(), new CartState.CartItem(
                            item.productId(), item.name(), e.getNewQuantity(), item.unitPriceCents()));
                    }
                } else if (typeUrl.endsWith("ItemRemoved")) {
                    ItemRemoved e = event.unpack(ItemRemoved.class);
                    items.remove(e.getProductId());
                } else if (typeUrl.endsWith("CouponApplied")) {
                    CouponApplied e = event.unpack(CouponApplied.class);
                    couponCode = e.getCouponCode();
                    discountCents = e.getDiscountCents();
                } else if (typeUrl.endsWith("CartCleared")) {
                    items.clear();
                    couponCode = "";
                    discountCents = 0;
                } else if (typeUrl.endsWith("CartCheckedOut")) {
                    status = "checked_out";
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack event: {}", typeUrl, e);
            }
        }

        int subtotal = items.values().stream()
            .mapToInt(item -> item.quantity() * item.unitPriceCents())
            .sum();

        return new CartState(customerId, items, subtotal, couponCode, discountCents, status);
    }

    private EventBook processCommand(ContextualCommand request)
            throws CommandValidationException, InvalidProtocolBufferException {
        CommandBook cmdBook = request.getCommand();
        EventBook priorEvents = request.getEvents();

        if (cmdBook == null || cmdBook.getPagesList().isEmpty()) {
            throw CommandValidationException.invalidArgument("CommandBook has no pages");
        }

        var cmdPage = cmdBook.getPages(0);
        if (!cmdPage.hasCommand()) {
            throw CommandValidationException.invalidArgument("Command page has no command");
        }

        CartState state = rebuildState(priorEvents);
        Any command = cmdPage.getCommand();
        String typeUrl = command.getTypeUrl();
        int seq = priorEvents != null ? priorEvents.getPagesCount() : 0;

        EventBook.Builder result = EventBook.newBuilder().setCover(cmdBook.getCover());

        if (typeUrl.endsWith("CreateCart")) {
            CreateCart cmd = command.unpack(CreateCart.class);
            handleCreateCart(state, cmd, result, seq);
        } else if (typeUrl.endsWith("AddItem")) {
            AddItem cmd = command.unpack(AddItem.class);
            handleAddItem(state, cmd, result, seq);
        } else if (typeUrl.endsWith("UpdateQuantity")) {
            UpdateQuantity cmd = command.unpack(UpdateQuantity.class);
            handleUpdateQuantity(state, cmd, result, seq);
        } else if (typeUrl.endsWith("RemoveItem")) {
            RemoveItem cmd = command.unpack(RemoveItem.class);
            handleRemoveItem(state, cmd, result, seq);
        } else if (typeUrl.endsWith("ApplyCoupon")) {
            ApplyCoupon cmd = command.unpack(ApplyCoupon.class);
            handleApplyCoupon(state, cmd, result, seq);
        } else if (typeUrl.endsWith("ClearCart")) {
            handleClearCart(state, result, seq);
        } else if (typeUrl.endsWith("Checkout")) {
            Checkout cmd = command.unpack(Checkout.class);
            handleCheckout(state, cmd, result, seq);
        } else {
            throw CommandValidationException.invalidArgument("Unknown command type: " + typeUrl);
        }

        return result.build();
    }

    private void handleCreateCart(CartState state, CreateCart cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Cart already exists");
        }
        if (cmd.getCustomerId().isBlank()) {
            throw CommandValidationException.invalidArgument("Customer ID is required");
        }

        logger.info("creating_cart", kv("customer_id", cmd.getCustomerId()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(CartCreated.newBuilder()
                .setCustomerId(cmd.getCustomerId())
                .setCreatedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleAddItem(CartState state, AddItem cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Cart does not exist");
        if (state.isCheckedOut()) throw CommandValidationException.failedPrecondition("Cannot modify checked out cart");
        if (cmd.getProductId().isBlank()) throw CommandValidationException.invalidArgument("Product ID is required");
        if (cmd.getQuantity() <= 0) throw CommandValidationException.invalidArgument("Quantity must be positive");
        if (cmd.getUnitPriceCents() <= 0) throw CommandValidationException.invalidArgument("Unit price must be positive");

        logger.info("adding_item", kv("product_id", cmd.getProductId()), kv("quantity", cmd.getQuantity()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(ItemAdded.newBuilder()
                .setProductId(cmd.getProductId())
                .setName(cmd.getName())
                .setQuantity(cmd.getQuantity())
                .setUnitPriceCents(cmd.getUnitPriceCents())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleUpdateQuantity(CartState state, UpdateQuantity cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Cart does not exist");
        if (state.isCheckedOut()) throw CommandValidationException.failedPrecondition("Cannot modify checked out cart");
        if (cmd.getProductId().isBlank()) throw CommandValidationException.invalidArgument("Product ID is required");
        if (cmd.getNewQuantity() <= 0) throw CommandValidationException.invalidArgument("Quantity must be positive");
        if (!state.items().containsKey(cmd.getProductId()))
            throw CommandValidationException.failedPrecondition("Item not in cart");

        var item = state.items().get(cmd.getProductId());
        logger.info("updating_quantity", kv("product_id", cmd.getProductId()),
            kv("old_qty", item.quantity()), kv("new_qty", cmd.getNewQuantity()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(QuantityUpdated.newBuilder()
                .setProductId(cmd.getProductId())
                .setOldQuantity(item.quantity())
                .setNewQuantity(cmd.getNewQuantity())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleRemoveItem(CartState state, RemoveItem cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Cart does not exist");
        if (state.isCheckedOut()) throw CommandValidationException.failedPrecondition("Cannot modify checked out cart");
        if (cmd.getProductId().isBlank()) throw CommandValidationException.invalidArgument("Product ID is required");
        if (!state.items().containsKey(cmd.getProductId()))
            throw CommandValidationException.failedPrecondition("Item not in cart");

        logger.info("removing_item", kv("product_id", cmd.getProductId()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(ItemRemoved.newBuilder()
                .setProductId(cmd.getProductId())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleApplyCoupon(CartState state, ApplyCoupon cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Cart does not exist");
        if (state.isCheckedOut()) throw CommandValidationException.failedPrecondition("Cannot modify checked out cart");
        if (!state.couponCode().isEmpty()) throw CommandValidationException.failedPrecondition("Coupon already applied");
        if (cmd.getCouponCode().isBlank()) throw CommandValidationException.invalidArgument("Coupon code is required");
        if (cmd.getDiscountCents() <= 0) throw CommandValidationException.invalidArgument("Discount must be positive");

        logger.info("applying_coupon", kv("coupon_code", cmd.getCouponCode()), kv("discount", cmd.getDiscountCents()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(CouponApplied.newBuilder()
                .setCouponCode(cmd.getCouponCode())
                .setDiscountCents(cmd.getDiscountCents())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleClearCart(CartState state, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Cart does not exist");
        if (state.isCheckedOut()) throw CommandValidationException.failedPrecondition("Cannot modify checked out cart");

        logger.info("clearing_cart", kv("customer_id", state.customerId()));

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(CartCleared.newBuilder()
                .setClearedAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleCheckout(CartState state, Checkout cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) throw CommandValidationException.failedPrecondition("Cart does not exist");
        if (state.isCheckedOut()) throw CommandValidationException.failedPrecondition("Cart already checked out");
        if (state.items().isEmpty()) throw CommandValidationException.failedPrecondition("Cannot checkout empty cart");

        logger.info("checking_out", kv("customer_id", state.customerId()), kv("item_count", state.items().size()));

        var items = state.items().values().stream()
            .map(item -> LineItem.newBuilder()
                .setProductId(item.productId())
                .setName(item.name())
                .setQuantity(item.quantity())
                .setUnitPriceCents(item.unitPriceCents())
                .build())
            .collect(Collectors.toList());

        int totalCents = state.subtotalCents() - state.discountCents();

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(CartCheckedOut.newBuilder()
                .setCustomerId(state.customerId())
                .addAllItems(items)
                .setSubtotalCents(state.subtotalCents())
                .setDiscountCents(state.discountCents())
                .setTotalCents(totalCents)
                .setLoyaltyPointsToUse(cmd.getLoyaltyPointsToUse())
                .setCheckedOutAt(nowTimestamp())
                .build()))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private Timestamp nowTimestamp() {
        Instant now = Instant.now();
        return Timestamp.newBuilder().setSeconds(now.getEpochSecond()).setNanos(now.getNano()).build();
    }
}
