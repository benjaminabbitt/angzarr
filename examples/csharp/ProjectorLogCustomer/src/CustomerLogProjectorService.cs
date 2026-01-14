using Angzarr;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Angzarr.Examples.Projector;

public class CustomerLogProjectorService : ProjectorCoordinator.ProjectorCoordinatorBase
{
    private readonly ICustomerLogProjector _projector;

    public CustomerLogProjectorService(ICustomerLogProjector projector)
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
