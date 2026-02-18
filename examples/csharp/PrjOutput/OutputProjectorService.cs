using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Grpc.Core;
using Angzarr;
using Angzarr.Examples;

namespace PrjOutput;

/// <summary>
/// gRPC service for the Output projector.
/// </summary>
public class OutputProjectorService : ProjectorService.ProjectorServiceBase
{
    private readonly OutputProjector _projector;
    private readonly StreamWriter? _logFile;

    public OutputProjectorService(string? logPath = null)
    {
        if (!string.IsNullOrEmpty(logPath))
        {
            _logFile = new StreamWriter(logPath, append: true);
            _projector = new OutputProjector(line =>
            {
                _logFile.WriteLine(line);
                _logFile.Flush();
            }, showTimestamps: true);
        }
        else
        {
            _projector = new OutputProjector(showTimestamps: true);
        }
    }

    public override Task<ComponentDescriptor> GetDescriptor(GetDescriptorRequest request, ServerCallContext context)
    {
        var descriptor = new ComponentDescriptor
        {
            Name = "projector-output",
            ComponentType = "projector"
        };

        // Subscribe to player domain
        var playerInput = new Target { Domain = "player" };
        playerInput.Types_.Add("PlayerRegistered");
        playerInput.Types_.Add("FundsDeposited");
        playerInput.Types_.Add("FundsWithdrawn");
        playerInput.Types_.Add("FundsReserved");
        playerInput.Types_.Add("FundsReleased");
        descriptor.Inputs.Add(playerInput);

        // Subscribe to table domain
        var tableInput = new Target { Domain = "table" };
        tableInput.Types_.Add("TableCreated");
        tableInput.Types_.Add("PlayerJoined");
        tableInput.Types_.Add("PlayerLeft");
        tableInput.Types_.Add("HandStarted");
        tableInput.Types_.Add("HandEnded");
        descriptor.Inputs.Add(tableInput);

        // Subscribe to hand domain
        var handInput = new Target { Domain = "hand" };
        handInput.Types_.Add("CardsDealt");
        handInput.Types_.Add("BlindPosted");
        handInput.Types_.Add("ActionTaken");
        handInput.Types_.Add("CommunityCardsDealt");
        handInput.Types_.Add("PotAwarded");
        handInput.Types_.Add("HandComplete");
        descriptor.Inputs.Add(handInput);

        return Task.FromResult(descriptor);
    }

    public override Task<Projection> Handle(EventBook request, ServerCallContext context)
    {
        var projection = _projector.Handle(request);
        return Task.FromResult(projection);
    }

    public override Task<Projection> HandleSpeculative(EventBook request, ServerCallContext context)
    {
        // Same as Handle for this simple projector
        var projection = _projector.Handle(request);
        return Task.FromResult(projection);
    }

    public void Close()
    {
        _logFile?.Close();
    }
}
