using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;

namespace Hand.SagaPlayer;

/// <summary>
/// gRPC service for Hand->Player saga.
/// </summary>
public class HandPlayerSagaService : SagaService.SagaServiceBase
{
    private readonly EventRouter _router;

    public HandPlayerSagaService(EventRouter router)
    {
        _router = router;
    }

    public override Task<SagaPrepareResponse> Prepare(SagaPrepareRequest request, ServerCallContext context)
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

    public override Task<SagaResponse> Execute(SagaExecuteRequest request, ServerCallContext context)
    {
        var response = new SagaResponse();

        foreach (var page in request.Source.Pages)
        {
            var eventMessage = UnpackEvent(page.Event);
            if (eventMessage == null) continue;

            var result = _router.DoHandle(eventMessage, request.Destinations.ToList());

            if (result is CommandBook commandBook)
            {
                response.Commands.Add(commandBook);
            }
            else if (result is List<CommandBook> commandBooks)
            {
                response.Commands.AddRange(commandBooks);
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
            "examples.PotAwarded" => eventAny.Unpack<PotAwarded>(),
            _ => null
        };
    }
}
