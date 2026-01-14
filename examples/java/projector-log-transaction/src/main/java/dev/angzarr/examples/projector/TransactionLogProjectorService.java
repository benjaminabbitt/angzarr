package dev.angzarr.examples.projector;

import com.google.protobuf.Empty;
import io.grpc.stub.StreamObserver;
import dev.angzarr.EventBook;
import dev.angzarr.Projection;
import dev.angzarr.ProjectorCoordinatorGrpc;

/**
 * gRPC service for transaction log projector.
 */
public class TransactionLogProjectorService extends ProjectorCoordinatorGrpc.ProjectorCoordinatorImplBase {

    private final TransactionLogProjector projector;

    public TransactionLogProjectorService(TransactionLogProjector projector) {
        this.projector = projector;
    }

    @Override
    public void handle(EventBook request, StreamObserver<Empty> responseObserver) {
        projector.logEvents(request);
        responseObserver.onNext(Empty.getDefaultInstance());
        responseObserver.onCompleted();
    }

    @Override
    public void handleSync(EventBook request, StreamObserver<Projection> responseObserver) {
        projector.logEvents(request);
        responseObserver.onNext(Projection.getDefaultInstance());
        responseObserver.onCompleted();
    }
}
