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
            if (page.Event != null)
            {
                var covers = _saga.Prepare(page.Event);
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
            if (page.Event == null)
                continue;

            var commands = _saga.Dispatch(page.Event, destinations: request.Destinations.ToList());
            response.Commands.AddRange(commands);
        }

        return Task.FromResult(response);
    }
}
// docs:end:saga_oo_service
