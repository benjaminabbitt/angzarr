using Angzarr;
using Angzarr.Client;
using Angzarr.Examples;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Table.SagaHandOO;

// docs:start:saga_oo_service
/// <summary>
/// gRPC service for Table->Hand saga using OO pattern.
/// </summary>
public class TableHandSagaService : SagaService.SagaServiceBase
{
    private readonly TableHandSaga _saga = new();

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
                var covers = _saga.Prepare(eventMessage);
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

        foreach (var page in request.Source.Pages)
        {
            var eventMessage = UnpackEvent(page.Event);
            if (eventMessage == null)
                continue;

            var result = _saga.Handle(eventMessage, request.Destinations.ToList());

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
// docs:end:saga_oo_service
