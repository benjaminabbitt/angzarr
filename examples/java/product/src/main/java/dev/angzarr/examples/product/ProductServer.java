package dev.angzarr.examples.product;

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

public class ProductServer extends BusinessLogicGrpc.BusinessLogicImplBase {
    private static final Logger logger = LoggerFactory.getLogger(ProductServer.class);
    private static final String DOMAIN = "product";
    private static final int DEFAULT_PORT = 50701;

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
            .addService(new ProductServer())
            .addService(health.getHealthService())
            .build();

        server.start();
        health.setStatus("", HealthCheckResponse.ServingStatus.SERVING);
        logger.info("Business logic server started: domain={}, port={}", DOMAIN, port);

        Runtime.getRuntime().addShutdownHook(new Thread(() -> {
            logger.info("Shutting down server...");
            server.shutdown();
        }));

        server.awaitTermination();
    }

    @Override
    public void handle(ContextualCommand request, StreamObserver<BusinessResponse> responseObserver) {
        try {
            EventBook events = processCommand(request);
            BusinessResponse response = BusinessResponse.newBuilder()
                .setEvents(events)
                .build();
            responseObserver.onNext(response);
            responseObserver.onCompleted();
        } catch (CommandValidationException e) {
            responseObserver.onError(Status.fromCode(e.getStatusCode())
                .withDescription(e.getMessage())
                .asRuntimeException());
        } catch (InvalidProtocolBufferException e) {
            responseObserver.onError(Status.INVALID_ARGUMENT
                .withDescription("Failed to parse command: " + e.getMessage())
                .asRuntimeException());
        } catch (Exception e) {
            logger.error("Unexpected error processing command", e);
            responseObserver.onError(Status.INTERNAL
                .withDescription("Internal error: " + e.getMessage())
                .asRuntimeException());
        }
    }

    private ProductState rebuildState(EventBook eventBook) {
        ProductState state = ProductState.empty();
        if (eventBook == null || eventBook.getPagesList().isEmpty()) {
            return state;
        }

        for (EventPage page : eventBook.getPagesList()) {
            if (!page.hasEvent()) continue;
            Any event = page.getEvent();
            String typeUrl = event.getTypeUrl();

            try {
                if (typeUrl.endsWith("ProductCreated")) {
                    ProductCreated e = event.unpack(ProductCreated.class);
                    state = new ProductState(e.getSku(), e.getName(), e.getDescription(), e.getPriceCents(), "active");
                } else if (typeUrl.endsWith("ProductUpdated")) {
                    ProductUpdated e = event.unpack(ProductUpdated.class);
                    state = new ProductState(state.sku(), e.getName(), e.getDescription(), state.priceCents(), state.status());
                } else if (typeUrl.endsWith("PriceSet")) {
                    PriceSet e = event.unpack(PriceSet.class);
                    state = new ProductState(state.sku(), state.name(), state.description(), e.getNewPriceCents(), state.status());
                } else if (typeUrl.endsWith("ProductDiscontinued")) {
                    state = new ProductState(state.sku(), state.name(), state.description(), state.priceCents(), "discontinued");
                }
            } catch (InvalidProtocolBufferException e) {
                logger.warn("Failed to unpack event: {}", typeUrl, e);
            }
        }
        return state;
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

        ProductState state = rebuildState(priorEvents);
        Any command = cmdPage.getCommand();
        String typeUrl = command.getTypeUrl();
        int seq = priorEvents != null ? priorEvents.getPagesCount() : 0;

        EventBook.Builder result = EventBook.newBuilder().setCover(cmdBook.getCover());

        if (typeUrl.endsWith("CreateProduct")) {
            CreateProduct cmd = command.unpack(CreateProduct.class);
            handleCreateProduct(state, cmd, result, seq);
        } else if (typeUrl.endsWith("UpdateProduct")) {
            UpdateProduct cmd = command.unpack(UpdateProduct.class);
            handleUpdateProduct(state, cmd, result, seq);
        } else if (typeUrl.endsWith("SetPrice")) {
            SetPrice cmd = command.unpack(SetPrice.class);
            handleSetPrice(state, cmd, result, seq);
        } else if (typeUrl.endsWith("Discontinue")) {
            handleDiscontinue(state, result, seq);
        } else {
            throw CommandValidationException.invalidArgument("Unknown command type: " + typeUrl);
        }

        return result.build();
    }

    private void handleCreateProduct(ProductState state, CreateProduct cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (state.exists()) {
            throw CommandValidationException.failedPrecondition("Product already exists");
        }
        if (cmd.getSku().isBlank()) {
            throw CommandValidationException.invalidArgument("Product SKU is required");
        }
        if (cmd.getName().isBlank()) {
            throw CommandValidationException.invalidArgument("Product name is required");
        }
        if (cmd.getPriceCents() <= 0) {
            throw CommandValidationException.invalidArgument("Price must be positive");
        }

        logger.info("creating_product", kv("sku", cmd.getSku()), kv("name", cmd.getName()));

        ProductCreated event = ProductCreated.newBuilder()
            .setSku(cmd.getSku())
            .setName(cmd.getName())
            .setDescription(cmd.getDescription())
            .setPriceCents(cmd.getPriceCents())
            .setCreatedAt(nowTimestamp())
            .build();

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(event))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleUpdateProduct(ProductState state, UpdateProduct cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Product does not exist");
        }
        if (state.isDiscontinued()) {
            throw CommandValidationException.failedPrecondition("Cannot update discontinued product");
        }
        if (cmd.getName().isBlank()) {
            throw CommandValidationException.invalidArgument("Product name is required");
        }

        logger.info("updating_product", kv("sku", state.sku()), kv("name", cmd.getName()));

        ProductUpdated event = ProductUpdated.newBuilder()
            .setName(cmd.getName())
            .setDescription(cmd.getDescription())
            .setUpdatedAt(nowTimestamp())
            .build();

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(event))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleSetPrice(ProductState state, SetPrice cmd, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Product does not exist");
        }
        if (state.isDiscontinued()) {
            throw CommandValidationException.failedPrecondition("Cannot set price on discontinued product");
        }
        if (cmd.getNewPriceCents() <= 0) {
            throw CommandValidationException.invalidArgument("Price must be positive");
        }

        logger.info("setting_price", kv("sku", state.sku()), kv("old_price", state.priceCents()), kv("new_price", cmd.getNewPriceCents()));

        PriceSet event = PriceSet.newBuilder()
            .setOldPriceCents(state.priceCents())
            .setNewPriceCents(cmd.getNewPriceCents())
            .setEffectiveAt(nowTimestamp())
            .build();

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(event))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private void handleDiscontinue(ProductState state, EventBook.Builder result, int seq)
            throws CommandValidationException {
        if (!state.exists()) {
            throw CommandValidationException.failedPrecondition("Product does not exist");
        }
        if (state.isDiscontinued()) {
            throw CommandValidationException.failedPrecondition("Product already discontinued");
        }

        logger.info("discontinuing_product", kv("sku", state.sku()));

        ProductDiscontinued event = ProductDiscontinued.newBuilder()
            .setDiscontinuedAt(nowTimestamp())
            .build();

        result.addPages(EventPage.newBuilder()
            .setNum(seq)
            .setEvent(Any.pack(event))
            .setCreatedAt(nowTimestamp())
            .build());
    }

    private Timestamp nowTimestamp() {
        Instant now = Instant.now();
        return Timestamp.newBuilder()
            .setSeconds(now.getEpochSecond())
            .setNanos(now.getNano())
            .build();
    }
}
