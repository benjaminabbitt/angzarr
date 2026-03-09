package dev.angzarr.examples.player;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import com.google.protobuf.Message;
import dev.angzarr.*;
import dev.angzarr.client.CommandRouter;
import dev.angzarr.client.Helpers;
import dev.angzarr.examples.*;
import dev.angzarr.examples.player.handlers.RejectedHandler;
import dev.angzarr.examples.player.handlers.StateBuilder;
import dev.angzarr.examples.player.state.PlayerState;
import io.grpc.stub.StreamObserver;
import net.devh.boot.grpc.server.service.GrpcService;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;

/**
 * Spring Boot application for Player aggregate (functional pattern).
 *
 * <p>Uses CommandRouter with standalone functional handlers following the guard/validate/compute
 * pattern.
 */
@SpringBootApplication
public class Main {

  public static void main(String[] args) {
    SpringApplication.run(Main.class, args);
  }

  @GrpcService
  public static class CommandHandlerService
      extends CommandHandlerServiceGrpc.CommandHandlerServiceImplBase {

    private static final String TYPE_URL_PREFIX = "type.googleapis.com/";

    private final CommandRouter<PlayerState> router = PlayerRouter.create();

    @Override
    public void handle(
        ContextualCommand request, StreamObserver<BusinessResponse> responseObserver) {
      try {
        // Check for notification (rejection/compensation)
        CommandBook commandBook = request.getCommand();
        if (commandBook != null && !commandBook.getPagesList().isEmpty()) {
          Any commandAny = commandBook.getPages(0).getCommand();
          if (commandAny != null && commandAny.getTypeUrl().endsWith("Notification")) {
            BusinessResponse response = handleNotification(request, commandAny);
            responseObserver.onNext(response);
            responseObserver.onCompleted();
            return;
          }
        }

        // Normal command dispatch
        EventBook events = router.dispatch(request);
        responseObserver.onNext(BusinessResponse.newBuilder().setEvents(events).build());
        responseObserver.onCompleted();
      } catch (CommandRouter.RouterException e) {
        responseObserver.onError(
            io.grpc.Status.FAILED_PRECONDITION.withDescription(e.getMessage()).asException());
      } catch (Exception e) {
        responseObserver.onError(
            io.grpc.Status.INTERNAL.withDescription(e.getMessage()).asException());
      }
    }

    private BusinessResponse handleNotification(ContextualCommand request, Any commandAny) {
      try {
        Notification notification = commandAny.unpack(Notification.class);

        // Extract target domain and command from rejection
        String targetDomain = "";
        String targetCommand = "";

        if (notification.hasPayload()) {
          try {
            RejectionNotification rejection =
                notification.getPayload().unpack(RejectionNotification.class);
            if (rejection.hasRejectedCommand()
                && rejection.getRejectedCommand().getPagesCount() > 0) {
              CommandBook rejectedCmd = rejection.getRejectedCommand();
              targetDomain = rejectedCmd.hasCover() ? rejectedCmd.getCover().getDomain() : "";
              targetCommand =
                  Helpers.typeNameFromUrl(rejectedCmd.getPages(0).getCommand().getTypeUrl());
            }
          } catch (InvalidProtocolBufferException ignored) {
            // Malformed rejection notification
          }
        }

        // Handle JoinTable rejection from table domain
        if ("table".equals(targetDomain) && targetCommand.endsWith("JoinTable")) {
          EventBook eventBook =
              request.hasEvents() ? request.getEvents() : EventBook.getDefaultInstance();
          PlayerState state = StateBuilder.fromEventBook(eventBook);
          int seq = Helpers.nextSequence(eventBook);

          Message event = RejectedHandler.handleJoinRejected(notification, state);
          EventBook resultBook =
              EventBook.newBuilder()
                  .setCover(request.getCommand().getCover())
                  .addPages(
                      EventPage.newBuilder()
                          .setHeader(PageHeader.newBuilder().setSequence(seq).build())
                          .setEvent(Any.pack(event, TYPE_URL_PREFIX))
                          .build())
                  .build();
          return BusinessResponse.newBuilder().setEvents(resultBook).build();
        }

        // Default: delegate to framework
        return BusinessResponse.newBuilder()
            .setRevocation(
                RevocationResponse.newBuilder()
                    .setEmitSystemRevocation(true)
                    .setSendToDeadLetterQueue(false)
                    .setEscalate(false)
                    .setAbort(false)
                    .setReason(
                        String.format(
                            "No handler for rejection %s/%s", targetDomain, targetCommand))
                    .build())
            .build();
      } catch (InvalidProtocolBufferException e) {
        return BusinessResponse.newBuilder()
            .setRevocation(
                RevocationResponse.newBuilder()
                    .setEmitSystemRevocation(true)
                    .setReason("Failed to decode notification: " + e.getMessage())
                    .build())
            .build();
      }
    }

    @Override
    public void replay(ReplayRequest request, StreamObserver<ReplayResponse> responseObserver) {
      // Build state from events
      PlayerState state =
          StateBuilder.fromEventBook(
              EventBook.newBuilder().addAllPages(request.getEventsList()).build());

      // Convert to proto state
      dev.angzarr.examples.PlayerState protoState =
          dev.angzarr.examples.PlayerState.newBuilder()
              .setPlayerId(state.getPlayerId())
              .setDisplayName(state.getDisplayName())
              .setEmail(state.getEmail())
              .setPlayerType(dev.angzarr.examples.PlayerType.forNumber(state.getPlayerType()))
              .setAiModelId(state.getAiModelId())
              .setBankroll(
                  Currency.newBuilder().setAmount(state.getBankroll()).setCurrencyCode("CHIPS"))
              .setReservedFunds(
                  Currency.newBuilder()
                      .setAmount(state.getReservedFunds())
                      .setCurrencyCode("CHIPS"))
              .putAllTableReservations(state.getTableReservations())
              .setStatus(state.getStatus())
              .build();

      responseObserver.onNext(
          ReplayResponse.newBuilder().setState(Any.pack(protoState, TYPE_URL_PREFIX)).build());
      responseObserver.onCompleted();
    }
  }
}
