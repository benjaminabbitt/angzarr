using Angzarr;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Angzarr.Examples.Projector;

public class TransactionLogProjectorService : ProjectorCoordinator.ProjectorCoordinatorBase
{
    private readonly ITransactionLogProjector _projector;

    public TransactionLogProjectorService(ITransactionLogProjector projector)
    {
        _projector = projector;
    }

    public override Task<Empty> Handle(EventBook request, ServerCallContext context)
    {
        _projector.LogEvents(request);
        return Task.FromResult(new Empty());
    }

    public override Task<Projection> HandleSync(EventBook request, ServerCallContext context)
    {
        _projector.LogEvents(request);
        // Log projector doesn't produce a projection
        return Task.FromResult<Projection>(null!);
    }
}
