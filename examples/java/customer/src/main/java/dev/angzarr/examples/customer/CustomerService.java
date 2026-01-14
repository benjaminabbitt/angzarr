package dev.angzarr.examples.customer;

import com.google.protobuf.Any;
import com.google.protobuf.InvalidProtocolBufferException;
import examples.Domains.AddLoyaltyPoints;
import examples.Domains.CreateCustomer;
import examples.Domains.RedeemLoyaltyPoints;
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
 * gRPC service implementation for customer business logic.
 * Uses dependency injection for the business logic implementation.
 */
public class CustomerService extends BusinessLogicGrpc.BusinessLogicImplBase {
    private static final Logger logger = LoggerFactory.getLogger(CustomerService.class);
    private static final String DOMAIN = "customer";

    private final CustomerLogic logic;

    public CustomerService(CustomerLogic logic) {
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

        CustomerState state = logic.rebuildState(priorEvents);
        Any command = cmdPage.getCommand();
        String typeUrl = command.getTypeUrl();

        EventBook result;
        if (typeUrl.endsWith("CreateCustomer")) {
            CreateCustomer cmd = command.unpack(CreateCustomer.class);
            result = logic.handleCreateCustomer(state, cmd.getName(), cmd.getEmail());
        } else if (typeUrl.endsWith("AddLoyaltyPoints")) {
            AddLoyaltyPoints cmd = command.unpack(AddLoyaltyPoints.class);
            result = logic.handleAddLoyaltyPoints(state, cmd.getPoints(), cmd.getReason());
        } else if (typeUrl.endsWith("RedeemLoyaltyPoints")) {
            RedeemLoyaltyPoints cmd = command.unpack(RedeemLoyaltyPoints.class);
            result = logic.handleRedeemLoyaltyPoints(state, cmd.getPoints(), cmd.getRedemptionType());
        } else {
            throw CommandValidationException.invalidArgument("Unknown command type: " + typeUrl);
        }

        // Add cover from command book to the result
        return result.toBuilder()
            .setCover(cmdBook.getCover())
            .build();
    }
}
