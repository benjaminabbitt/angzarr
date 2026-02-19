using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Angzarr;
using Angzarr.Examples;

namespace HandFlow;

/// <summary>
/// gRPC service for Hand Flow process manager.
/// </summary>
public class HandFlowService : ProcessManagerService.ProcessManagerServiceBase
{
    private readonly HandFlowProcessManager _pm;

    public HandFlowService(HandFlowProcessManager pm)
    {
        _pm = pm;
    }

    public override Task<ProcessManagerPrepareResponse> Prepare(ProcessManagerPrepareRequest request, ServerCallContext context)
    {
        var covers = _pm.Prepare(request.Trigger, request.ProcessState);

        var response = new ProcessManagerPrepareResponse();
        response.Destinations.AddRange(covers);
        return Task.FromResult(response);
    }

    public override Task<ProcessManagerHandleResponse> Handle(ProcessManagerHandleRequest request, ServerCallContext context)
    {
        var (commands, events) = _pm.Handle(request.Trigger, request.ProcessState, request.Destinations.ToList());

        var response = new ProcessManagerHandleResponse();
        response.Commands.AddRange(commands);
        if (events != null)
        {
            response.ProcessEvents = events;
        }

        return Task.FromResult(response);
    }
}
