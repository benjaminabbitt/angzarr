package dev.angzarr.examples.player.sagatable;

import dev.angzarr.*;
import dev.angzarr.client.router.SagaHandlerResponse;
import io.grpc.stub.StreamObserver;
import java.util.List;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Player -> Table saga.
 *
 * <p>Propagates player sit-out/sit-in intent as facts to the table domain. Uses the
 * annotation-based OO Saga pattern.
 */
@SpringBootApplication
public class Main {

  public static void main(String[] args) {
    SpringApplication.run(Main.class, args);
  }

  @GrpcService
  public static class SagaService extends SagaServiceGrpc.SagaServiceImplBase {
    private final PlayerTableSaga saga = new PlayerTableSaga();

    @Override
    public void handle(SagaHandleRequest request, StreamObserver<SagaResponse> responseObserver) {
      // Dispatch using the annotation-based saga pattern
      SagaHandlerResponse response = saga.dispatch(request.getSource(), List.of());

      SagaResponse.Builder builder =
          SagaResponse.newBuilder().addAllCommands(response.getCommands());

      if (response.hasEvents()) {
        builder.addAllEvents(response.getEvents());
      }

      responseObserver.onNext(builder.build());
      responseObserver.onCompleted();
    }
  }
}
