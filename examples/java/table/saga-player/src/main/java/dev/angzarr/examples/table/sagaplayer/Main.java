package dev.angzarr.examples.table.sagaplayer;

import dev.angzarr.*;
import dev.angzarr.client.EventRouter;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Table -> Player saga.
 *
 * <p>Uses the functional EventRouter pattern.
 */
@SpringBootApplication
public class Main {

  public static void main(String[] args) {
    SpringApplication.run(Main.class, args);
  }

  @GrpcService
  public static class SagaService extends SagaServiceGrpc.SagaServiceImplBase {
    private final EventRouter router = TablePlayerRouter.createRouter();

    @Override
    public void prepare(
        SagaPrepareRequest request, StreamObserver<SagaPrepareResponse> responseObserver) {
      var destinations = router.prepareDestinations(request.getSource());
      responseObserver.onNext(
          SagaPrepareResponse.newBuilder().addAllDestinations(destinations).build());
      responseObserver.onCompleted();
    }

    @Override
    public void execute(SagaExecuteRequest request, StreamObserver<SagaResponse> responseObserver) {
      var commands = router.dispatch(request.getSource(), request.getDestinationsList());
      responseObserver.onNext(SagaResponse.newBuilder().addAllCommands(commands).build());
      responseObserver.onCompleted();
    }
  }
}
