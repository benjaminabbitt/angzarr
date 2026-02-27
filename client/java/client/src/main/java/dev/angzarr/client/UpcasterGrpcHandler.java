package dev.angzarr.client;

import dev.angzarr.*;
import io.grpc.stub.StreamObserver;

import java.util.List;

/**
 * gRPC service handler for upcaster.
 *
 * <p>Wraps an {@link UpcasterRouter} and implements the gRPC UpcasterService.
 * This can be used directly with Spring Boot's @GrpcService annotation or
 * with a standalone gRPC server.
 *
 * <h2>Spring Boot Usage</h2>
 * <pre>{@code
 * @GrpcService
 * public class PlayerUpcasterService extends UpcasterGrpcHandler {
 *     public PlayerUpcasterService() {
 *         super(new UpcasterRouter("player")
 *             .on("PlayerRegisteredV1", old -> {
 *                 PlayerRegisteredV1 v1 = old.unpack(PlayerRegisteredV1.class);
 *                 return Any.pack(PlayerRegistered.newBuilder()
 *                     .setDisplayName(v1.getDisplayName())
 *                     .build());
 *             }));
 *     }
 * }
 * }</pre>
 *
 * <h2>Standalone Server Usage</h2>
 * <pre>{@code
 * UpcasterRouter router = new UpcasterRouter("player")
 *     .on("PlayerRegisteredV1", transformer);
 *
 * Server server = ServerBuilder.forPort(50401)
 *     .addService(new UpcasterGrpcHandler(router))
 *     .build()
 *     .start();
 * }</pre>
 */
public class UpcasterGrpcHandler extends UpcasterServiceGrpc.UpcasterServiceImplBase {
    private final UpcasterRouter router;

    /**
     * Create a new upcaster gRPC handler.
     *
     * @param router The upcaster router to use for transformations
     */
    public UpcasterGrpcHandler(UpcasterRouter router) {
        this.router = router;
    }

    /**
     * Get the underlying router.
     *
     * @return The upcaster router
     */
    public UpcasterRouter getRouter() {
        return router;
    }

    /**
     * Get the domain this handler serves.
     *
     * @return The domain name
     */
    public String getDomain() {
        return router.getDomain();
    }

    @Override
    public void upcast(UpcastRequest request, StreamObserver<UpcastResponse> responseObserver) {
        List<EventPage> events = router.upcast(request.getEventsList());
        responseObserver.onNext(UpcastResponse.newBuilder().addAllEvents(events).build());
        responseObserver.onCompleted();
    }
}
