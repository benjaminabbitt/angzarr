package dev.angzarr.examples.transaction;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import examples.Domains.*;
import io.grpc.Status;
import io.grpc.stub.StreamObserver;
import dev.angzarr.BusinessLogicGrpc;
import dev.angzarr.BusinessResponse;
import dev.angzarr.CommandBook;
import dev.angzarr.ContextualCommand;
import dev.angzarr.EventBook;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;

/**
 * gRPC service implementation for transaction business logic.
 */
public class TransactionService extends BusinessLogicGrpc.BusinessLogicImplBase {
    private static final Logger logger = LoggerFactory.getLogger(TransactionService.class);

    private final TransactionLogic logic;

    public TransactionService(TransactionLogic logic) {
        this.logic = logic;
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

        TransactionState state = logic.rebuildState(priorEvents);
        Any command = cmdPage.getCommand();
        String typeUrl = command.getTypeUrl();

        EventBook result;
        if (typeUrl.endsWith("CreateTransaction")) {
            CreateTransaction cmd = command.unpack(CreateTransaction.class);
            result = logic.handleCreateTransaction(state, cmd.getCustomerId(), cmd.getItemsList());
        } else if (typeUrl.endsWith("ApplyDiscount")) {
            ApplyDiscount cmd = command.unpack(ApplyDiscount.class);
            result = logic.handleApplyDiscount(state, cmd.getDiscountType(), cmd.getValue(), cmd.getCouponCode());
        } else if (typeUrl.endsWith("CompleteTransaction")) {
            CompleteTransaction cmd = command.unpack(CompleteTransaction.class);
            result = logic.handleCompleteTransaction(state, cmd.getPaymentMethod());
        } else if (typeUrl.endsWith("CancelTransaction")) {
            CancelTransaction cmd = command.unpack(CancelTransaction.class);
            result = logic.handleCancelTransaction(state, cmd.getReason());
        } else {
            throw CommandValidationException.invalidArgument("Unknown command type: " + typeUrl);
        }

        return result.toBuilder()
            .setCover(cmdBook.getCover())
            .build();
    }
}
