package dev.angzarr.examples.hand.sagatable;

import dev.angzarr.*;
import dev.angzarr.client.EventRouter;
import io.grpc.stub.StreamObserver;
import java.util.List;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Hand -> Table saga.
 *
 * <p>Uses the functional EventRouter pattern. Sagas are stateless translators - framework handles
 * sequence stamping.
 */
@SpringBootApplication
public class Main {

  public static void main(String[] args) {
    SpringApplication.run(Main.class, args);
  }

  @GrpcService
  public static class SagaService extends SagaServiceGrpc.SagaServiceImplBase {
    private final EventRouter router = HandTableRouter.createRouter();

    @Override
    public void handle(SagaHandleRequest request, StreamObserver<SagaResponse> responseObserver) {
      // Sagas receive source events only - framework handles destinations and sequences
      var commands = router.dispatch(request.getSource(), List.of());
      responseObserver.onNext(SagaResponse.newBuilder().addAllCommands(commands).build());
      responseObserver.onCompleted();
    }
  }
}
