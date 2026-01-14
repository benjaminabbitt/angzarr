using Angzarr;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Angzarr.Examples.Projector;

public class ReceiptProjectorService : Angzarr.Projector.ProjectorBase
{
    private readonly IReceiptProjector _projector;

    public ReceiptProjectorService(IReceiptProjector projector)
    {
        _projector = projector;
    }

    public override Task<Empty> Handle(EventBook request, ServerCallContext context)
    {
        _projector.Project(request);
        return Task.FromResult(new Empty());
    }

    public override Task<Projection> HandleSync(EventBook request, ServerCallContext context)
    {
        var result = _projector.Project(request);
        return Task.FromResult(result ?? new Projection());
    }
}
