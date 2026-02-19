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
