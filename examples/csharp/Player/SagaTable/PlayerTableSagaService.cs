using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Player.SagaTable;

/// <summary>
/// gRPC service for Player->Table saga.
///
/// Emits facts (events) to table domain for sit-out/sit-in tracking.
/// </summary>
public class PlayerTableSagaService : SagaService.SagaServiceBase
{
    private readonly EventRouter _router;

    public PlayerTableSagaService(EventRouter router)
    {
        _router = router;
    }

    public override Task<SagaPrepareResponse> Prepare(
        SagaPrepareRequest request,
        ServerCallContext context
    )
    {
        var response = new SagaPrepareResponse();

        foreach (var page in request.Source.Pages)
        {
            var eventMessage = UnpackEvent(page.Event);
            if (eventMessage != null)
            {
                var covers = _router.DoPrepare(eventMessage);
                response.Destinations.AddRange(covers);
            }
        }

        return Task.FromResult(response);
    }

    public override Task<SagaResponse> Execute(
        SagaExecuteRequest request,
        ServerCallContext context
    )
    {
        var response = new SagaResponse();

        // Set source root for handler access
        PlayerTableSaga.SetSourceRoot(request.Source);

        foreach (var page in request.Source.Pages)
        {
            var eventMessage = UnpackEvent(page.Event);
            if (eventMessage == null)
                continue;

            var result = _router.DoHandle(eventMessage, request.Destinations.ToList());

            // This saga emits facts (EventBooks), not commands
            if (result is EventBook eventBook)
            {
                response.Events.Add(eventBook);
            }
            else if (result is List<EventBook> eventBooks)
            {
                response.Events.AddRange(eventBooks);
            }
        }

        return Task.FromResult(response);
    }

    private static IMessage? UnpackEvent(Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;
        var typeName = typeUrl.Contains('/') ? typeUrl.Split('/').Last() : typeUrl;

        return typeName switch
        {
            "examples.PlayerSittingOut" => eventAny.Unpack<PlayerSittingOut>(),
            "examples.PlayerReturningToPlay" => eventAny.Unpack<PlayerReturningToPlay>(),
            _ => null,
        };
    }
}
