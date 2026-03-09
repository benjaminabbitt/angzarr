using Angzarr;
using Angzarr.Client;
using Grpc.Core;

namespace PrjOutputOO;

// docs:start:projector_oo_service
/// <summary>
/// gRPC service for Output projector using OO pattern.
/// </summary>
public class OutputProjectorService : ProjectorService.ProjectorServiceBase
{
    private readonly OutputProjector _projector = new();

    public override Task<Projection> Handle(EventBook request, ServerCallContext context)
    {
        var lastProjection = new Projection();

        foreach (var page in request.Pages)
        {
            if (page.Event != null)
            {
                var result = _projector.Dispatch(page.Event);
                if (!string.IsNullOrEmpty(result.Projector))
                {
                    lastProjection = result;
                    lastProjection.Cover = request.Cover;
                    lastProjection.Sequence = page.Header?.Sequence ?? 0;
                }
            }
        }

        return Task.FromResult(lastProjection);
    }

    public override Task<Projection> HandleSpeculative(EventBook request, ServerCallContext context)
    {
        return Handle(request, context);
    }
}
// docs:end:projector_oo_service
