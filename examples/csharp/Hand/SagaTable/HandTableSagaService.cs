using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Hand.SagaTable;

/// <summary>
/// gRPC service for Hand->Table saga.
/// Sagas are stateless translators - framework handles sequence stamping.
/// </summary>
public class HandTableSagaService : SagaService.SagaServiceBase
{
    private readonly EventRouter _router;

    public HandTableSagaService(EventRouter router)
    {
        _router = router;
    }

    public override Task<SagaResponse> Handle(SagaHandleRequest request, ServerCallContext context)
    {
        var response = new SagaResponse();

        foreach (var page in request.Source.Pages)
        {
            var eventMessage = UnpackEvent(page.Event);
            if (eventMessage == null)
                continue;

            // Sagas receive source events only - framework handles destinations
            var result = _router.DoHandle(eventMessage, new List<EventBook>());

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
            "examples.HandComplete" => eventAny.Unpack<HandComplete>(),
            _ => null,
        };
    }
}
