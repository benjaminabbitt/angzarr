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
/// Sagas are stateless translators - framework handles sequence stamping.
/// </summary>
public class TableHandSagaService : SagaService.SagaServiceBase
{
    private readonly TableHandSaga _saga = new();

    public override Task<SagaResponse> Handle(SagaHandleRequest request, ServerCallContext context)
    {
        var response = new SagaResponse();

        foreach (var page in request.Source.Pages)
        {
            if (page.Event == null)
                continue;

            // Sagas receive source events only - framework handles destinations
            var commands = _saga.Dispatch(page.Event, destinations: new List<EventBook>());
            response.Commands.AddRange(commands);
        }

        return Task.FromResult(response);
    }
}
// docs:end:saga_oo_service
