using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Table.SagaHand;

/// <summary>
/// gRPC service for Table->Hand saga.
/// Sagas are stateless translators - framework handles sequence stamping.
/// </summary>
public class TableHandSagaService : SagaService.SagaServiceBase
{
    private readonly EventRouter _router;

    public TableHandSagaService(EventRouter router)
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
            "examples.HandStarted" => eventAny.Unpack<HandStarted>(),
            _ => null,
        };
    }
}
