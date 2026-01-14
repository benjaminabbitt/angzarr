using Angzarr;
using Grpc.Net.Client;

namespace Integration.Tests.StepDefinitions;

public class TestContext : IDisposable
{
    public GrpcChannel? Channel { get; set; }
    public string? AngzarrHost { get; set; }
    public int AngzarrPort { get; set; }
    public Guid? CurrentCustomerId { get; set; }
    public Guid? CurrentTransactionId { get; set; }
    public CommandResponse? LastResponse { get; set; }
    public EventBook? LastEventBook { get; set; }
    public Exception? LastException { get; set; }

    public void Reset()
    {
        CurrentCustomerId = null;
        CurrentTransactionId = null;
        LastResponse = null;
        LastEventBook = null;
        LastException = null;
    }

    public void Dispose()
    {
        Channel?.Dispose();
    }
}
