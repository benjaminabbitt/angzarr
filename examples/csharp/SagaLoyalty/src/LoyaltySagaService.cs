using Angzarr;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;

namespace Angzarr.Examples.Saga;

public class LoyaltySagaService : Angzarr.Saga.SagaBase
{
    private readonly ILoyaltySaga _saga;

    public LoyaltySagaService(ILoyaltySaga saga)
    {
        _saga = saga;
    }

    public override Task<Empty> Handle(EventBook request, ServerCallContext context)
    {
        _saga.ProcessEvents(request);
        return Task.FromResult(new Empty());
    }

    public override Task<SagaResponse> HandleSync(EventBook request, ServerCallContext context)
    {
        var commands = _saga.ProcessEvents(request);

        var response = new SagaResponse();
        response.Commands.AddRange(commands);

        return Task.FromResult(response);
    }
}
