package dev.angzarr.examples.projector;

import com.google.protobuf.Empty;
import io.grpc.stub.StreamObserver;
import dev.angzarr.EventBook;
import dev.angzarr.Projection;
import dev.angzarr.ProjectorGrpc;

/**
 * gRPC service implementation for receipt projector.
 */
public class ReceiptProjectorService extends ProjectorGrpc.ProjectorImplBase {

    private final ReceiptProjector projector;

    public ReceiptProjectorService(ReceiptProjector projector) {
        this.projector = projector;
    }

    @Override
    public void handle(EventBook request, StreamObserver<Empty> responseObserver) {
        projector.project(request);
        responseObserver.onNext(Empty.getDefaultInstance());
        responseObserver.onCompleted();
    }

    @Override
    public void handleSync(EventBook request, StreamObserver<Projection> responseObserver) {
        Projection projection = projector.project(request);
        if (projection != null) {
            responseObserver.onNext(projection);
        } else {
            responseObserver.onNext(Projection.getDefaultInstance());
        }
        responseObserver.onCompleted();
    }
}
