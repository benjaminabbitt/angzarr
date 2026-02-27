package dev.angzarr.examples.player.upc;

import dev.angzarr.*;
import dev.angzarr.client.UpcasterRouter;
import io.grpc.stub.StreamObserver;
import java.util.List;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Player domain upcaster.
 *
 * <p>Transforms old event versions to current versions during replay. This is a passthrough
 * upcaster - no transformations yet.
 *
 * <h2>Adding Transformations</h2>
 *
 * <p>When schema evolution is needed, add transformations to the router:
 *
 * <pre>{@code
 * private static UpcasterRouter createRouter() {
 *     return new UpcasterRouter("player")
 *         .on("PlayerRegisteredV1", old -> {
 *             PlayerRegisteredV1 v1 = old.unpack(PlayerRegisteredV1.class);
 *             return Any.pack(PlayerRegistered.newBuilder()
 *                 .setDisplayName(v1.getDisplayName())
 *                 .setEmail(v1.getEmail())
 *                 .setPlayerType(v1.getPlayerType())
 *                 .setAiModelId("")  // New field with default
 *                 .build());
 *         });
 * }
 * }</pre>
 */
@SpringBootApplication
public class Main {

  public static void main(String[] args) {
    SpringApplication.run(Main.class, args);
  }

  // docs:start:upcaster_router
  /**
   * Create the upcaster router for player domain.
   *
   * <p>Currently a passthrough - add transformations as needed for schema evolution.
   */
  private static UpcasterRouter createRouter() {
    return new UpcasterRouter("player");
    // Example transformation (uncomment when needed):
    // .on("PlayerRegisteredV1", old -> {
    //     PlayerRegisteredV1 v1 = old.unpack(PlayerRegisteredV1.class);
    //     return Any.pack(PlayerRegistered.newBuilder()
    //         .setDisplayName(v1.getDisplayName())
    //         .setEmail(v1.getEmail())
    //         .setPlayerType(v1.getPlayerType())
    //         .setAiModelId("")
    //         .build());
    // });
  }

  // docs:end:upcaster_router

  // docs:start:upcaster_service
  @GrpcService
  public static class UpcasterService extends UpcasterServiceGrpc.UpcasterServiceImplBase {
    private final UpcasterRouter router = createRouter();

    @Override
    public void upcast(UpcastRequest request, StreamObserver<UpcastResponse> responseObserver) {
      List<EventPage> events = router.upcast(request.getEventsList());
      responseObserver.onNext(UpcastResponse.newBuilder().addAllEvents(events).build());
      responseObserver.onCompleted();
    }
  }
  // docs:end:upcaster_service
}
