package dev.angzarr.examples.table.sagahandoo;

import dev.angzarr.*;
import dev.angzarr.client.Saga;
import dev.angzarr.client.router.SagaHandlerResponse;
import io.grpc.stub.StreamObserver;
import java.util.List;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Table -> Hand saga using OO pattern.
 *
 * <p>This example demonstrates using the {@link Saga} base class with annotation-based handler
 * registration (@Prepares, @Handles).
 *
 * <p>Compare with the functional EventRouter pattern in table/saga-hand.
 */
@SpringBootApplication
public class Main {

  public static void main(String[] args) {
    SpringApplication.run(Main.class, args);
  }

  // docs:start:saga_oo_main
  @GrpcService
  public static class SagaService extends SagaServiceGrpc.SagaServiceImplBase {
    private final TableHandSaga saga = new TableHandSaga();

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
  // docs:end:saga_oo_main
}
