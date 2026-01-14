package dev.angzarr.examples.saga;

import com.google.protobuf.Empty;
import io.grpc.stub.StreamObserver;
import dev.angzarr.CommandBook;
import dev.angzarr.EventBook;
import dev.angzarr.SagaGrpc;
import dev.angzarr.SagaResponse;

import java.util.List;

/**
 * gRPC service for loyalty points saga.
 */
public class LoyaltySagaService extends SagaGrpc.SagaImplBase {

    private final LoyaltySaga saga;

    public LoyaltySagaService(LoyaltySaga saga) {
        this.saga = saga;
    }

    @Override
    public void handle(EventBook request, StreamObserver<Empty> responseObserver) {
        saga.processEvents(request);
        responseObserver.onNext(Empty.getDefaultInstance());
        responseObserver.onCompleted();
    }

    @Override
    public void handleSync(EventBook request, StreamObserver<SagaResponse> responseObserver) {
        List<CommandBook> commands = saga.processEvents(request);

        SagaResponse response = SagaResponse.newBuilder()
            .addAllCommands(commands)
            .build();

        responseObserver.onNext(response);
        responseObserver.onCompleted();
    }
}
