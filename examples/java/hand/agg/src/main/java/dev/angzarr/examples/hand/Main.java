package dev.angzarr.examples.hand;

import com.google.protobuf.ByteString;
import dev.angzarr.*;
import dev.angzarr.client.CommandHandler;
import dev.angzarr.examples.BettingPhase;
import dev.angzarr.examples.GameVariant;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Hand aggregate (OO pattern).
 *
 * <p>Uses CommandHandler with annotation-based dispatch (@Handles, @Applies).
 */
@SpringBootApplication
public class Main {

  public static void main(String[] args) {
    SpringApplication.run(Main.class, args);
  }

  @GrpcService
  public static class CommandHandlerService
      extends CommandHandlerServiceGrpc.CommandHandlerServiceImplBase {

    @Override
    public void handle(
        ContextualCommand request, StreamObserver<BusinessResponse> responseObserver) {
      try {
        BusinessResponse response = CommandHandler.handle(Hand.class, request);
        responseObserver.onNext(response);
        responseObserver.onCompleted();
      } catch (Exception e) {
        responseObserver.onError(
            io.grpc.Status.INTERNAL.withDescription(e.getMessage()).asException());
      }
    }

    @Override
    public void replay(ReplayRequest request, StreamObserver<ReplayResponse> responseObserver) {
      // Build state from events using the OO handler
      Hand handler = new Hand();
      handler.rehydrate(EventBook.newBuilder().addAllPages(request.getEventsList()).build());

      // Get state and convert to proto
      var state = handler.getState();
      var protoState =
          dev.angzarr.examples.HandState.newBuilder()
              .setHandId(state.getHandId() != null ? state.getHandId() : "")
              .setTableRoot(ByteString.copyFrom(state.getTableRoot()))
              .setHandNumber(state.getHandNumber())
              .setGameVariant(GameVariant.forNumber(state.getGameVariant()))
              .setCurrentPhase(BettingPhase.forNumber(state.getCurrentPhase()))
              .setActionOnPosition(state.getActionOnPosition())
              .setCurrentBet(state.getCurrentBet())
              .setMinRaise(state.getMinRaise())
              .setDealerPosition(state.getDealerPosition())
              .setSmallBlindPosition(state.getSmallBlindPosition())
              .setBigBlindPosition(state.getBigBlindPosition())
              .setStatus(state.getStatus() != null ? state.getStatus() : "")
              .build();

      responseObserver.onNext(
          ReplayResponse.newBuilder()
              .setState(com.google.protobuf.Any.pack(protoState, "type.googleapis.com/"))
              .build());
      responseObserver.onCompleted();
    }
  }
}
