using Grpc.Core;
using Angzarr;
using Angzarr.Client;

namespace HandFlowOO;

/// <summary>
/// gRPC service for Hand Flow process manager (OO pattern).
/// </summary>
public class HandFlowService : ProcessManagerService.ProcessManagerServiceBase
{
    public override Task<ComponentDescriptor> GetDescriptor(GetDescriptorRequest request, ServerCallContext context)
    {
        var pm = new HandFlowPM();
        var desc = pm.Descriptor();

        var descriptor = new ComponentDescriptor
        {
            Name = desc.Name,
            ComponentType = desc.ComponentType
        };

        foreach (var input in desc.Inputs)
        {
            var target = new Target { Domain = input.Domain };
            target.Types_.AddRange(input.Types);
            descriptor.Inputs.Add(target);
        }

        return Task.FromResult(descriptor);
    }

    public override Task<ProcessManagerPrepareResponse> Prepare(ProcessManagerPrepareRequest request, ServerCallContext context)
    {
        var covers = ProcessManager<PMState>.PrepareDestinations<HandFlowPM>(
            request.Trigger,
            request.ProcessState);

        var response = new ProcessManagerPrepareResponse();
        response.Destinations.AddRange(covers);
        return Task.FromResult(response);
    }

    public override Task<ProcessManagerHandleResponse> Handle(ProcessManagerHandleRequest request, ServerCallContext context)
    {
        var (commands, events) = ProcessManager<PMState>.Handle<HandFlowPM>(
            request.Trigger,
            request.ProcessState,
            request.Destinations.ToList());

        var response = new ProcessManagerHandleResponse();
        response.Commands.AddRange(commands);
        if (events != null)
        {
            response.ProcessEvents = events;
        }

        return Task.FromResult(response);
    }
}
