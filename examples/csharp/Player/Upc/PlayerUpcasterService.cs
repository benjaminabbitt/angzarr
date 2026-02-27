using Angzarr;
using Angzarr.Client;
using Grpc.Core;

namespace Player.Upc;

// docs:start:upcaster_service
/// <summary>
/// gRPC service for Player domain upcaster.
///
/// Transforms old event versions to current versions during replay.
/// This is a passthrough upcaster - no transformations yet.
/// </summary>
public class PlayerUpcasterService : UpcasterService.UpcasterServiceBase
{
    private readonly UpcasterRouter _router;

    public PlayerUpcasterService(UpcasterRouter router)
    {
        _router = router;
    }

    public override Task<UpcastResponse> Upcast(UpcastRequest request, ServerCallContext context)
    {
        var events = _router.Upcast(request.Events);

        var response = new UpcastResponse();
        response.Events.AddRange(events);
        return Task.FromResult(response);
    }
}
// docs:end:upcaster_service
