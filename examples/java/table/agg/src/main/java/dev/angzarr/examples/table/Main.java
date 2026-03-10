package dev.angzarr.examples.table;

import dev.angzarr.*;
import dev.angzarr.client.CommandHandler;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Table aggregate (OO pattern).
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
        BusinessResponse response = CommandHandler.handle(Table.class, request);
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
      Table handler = new Table();
      handler.rehydrate(EventBook.newBuilder().addAllPages(request.getEventsList()).build());

      // Get state and convert to proto
      var state = handler.getState();
      var protoState =
          dev.angzarr.examples.TableState.newBuilder()
              .setTableId(state.getTableId() != null ? state.getTableId() : "")
              .setTableName(state.getTableName() != null ? state.getTableName() : "")
              .setGameVariant(dev.angzarr.examples.GameVariant.forNumber(state.getGameVariant()))
              .setSmallBlind(state.getSmallBlind())
              .setBigBlind(state.getBigBlind())
              .setMinBuyIn(state.getMinBuyIn())
              .setMaxBuyIn(state.getMaxBuyIn())
              .setMaxPlayers(state.getMaxPlayers())
              .setActionTimeoutSeconds(state.getActionTimeoutSeconds())
              .setStatus(state.getStatus() != null ? state.getStatus() : "")
              .setDealerPosition(state.getDealerPosition())
              .setHandCount(state.getHandCount())
              .build();

      responseObserver.onNext(
          ReplayResponse.newBuilder()
              .setState(com.google.protobuf.Any.pack(protoState, "type.googleapis.com/"))
              .build());
      responseObserver.onCompleted();
    }
  }
}
