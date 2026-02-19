using Grpc.Core;
using Grpc.Net.Client;

namespace Angzarr.Client;

/// <summary>
/// Client for speculative (what-if) execution across coordinator types.
/// </summary>
/// <remarks>
/// Speculative execution allows testing commands, events, and projections
/// without persisting results. Use this for:
/// <list type="bullet">
///   <item>Form validation: "Will this order succeed?"</item>
///   <item>Preview: "What events would this produce?"</item>
///   <item>Testing: Verify business logic without polluting event store</item>
/// </list>
/// </remarks>
/// <example>
/// <code>
/// using var client = SpeculativeClient.Connect("http://localhost:1310");
/// var response = client.Aggregate(new SpeculateAggregateRequest
/// {
///     Command = command,
///     Events = priorEvents
/// });
/// </code>
/// </example>
public class SpeculativeClient : IDisposable
{
    private readonly AggregateCoordinatorService.AggregateCoordinatorServiceClient _aggregateStub;
    private readonly SagaCoordinatorService.SagaCoordinatorServiceClient _sagaStub;
    private readonly ProjectorCoordinatorService.ProjectorCoordinatorServiceClient _projectorStub;
    private readonly ProcessManagerCoordinatorService.ProcessManagerCoordinatorServiceClient _pmStub;
    private readonly GrpcChannel _channel;

    private SpeculativeClient(GrpcChannel channel)
    {
        _channel = channel;
        _aggregateStub = new AggregateCoordinatorService.AggregateCoordinatorServiceClient(channel);
        _sagaStub = new SagaCoordinatorService.SagaCoordinatorServiceClient(channel);
        _projectorStub = new ProjectorCoordinatorService.ProjectorCoordinatorServiceClient(channel);
        _pmStub = new ProcessManagerCoordinatorService.ProcessManagerCoordinatorServiceClient(channel);
    }

    /// <summary>
    /// Connect to coordinator services at the given endpoint.
    /// </summary>
    /// <param name="endpoint">The endpoint URL (e.g., "http://localhost:1310")</param>
    /// <returns>A connected SpeculativeClient</returns>
    /// <exception cref="ConnectionError">If connection fails</exception>
    public static SpeculativeClient Connect(string endpoint)
    {
        try
        {
            // Validate endpoint format
            if (!Uri.TryCreate(endpoint, UriKind.Absolute, out var uri))
            {
                // Try adding http:// prefix
                if (!Uri.TryCreate($"http://{endpoint}", UriKind.Absolute, out uri))
                {
                    throw new ConnectionError($"Invalid endpoint format: {endpoint}");
                }
            }

            // Validate it looks like a proper endpoint
            if (uri.Segments.Length > 1 || string.IsNullOrEmpty(uri.Host))
            {
                throw new ConnectionError($"Invalid endpoint format: {endpoint}");
            }

            var channel = GrpcChannel.ForAddress(uri);
            return new SpeculativeClient(channel);
        }
        catch (UriFormatException ex)
        {
            throw new ConnectionError($"Invalid endpoint format: {endpoint}", ex);
        }
    }

    /// <summary>
    /// Connect using an endpoint from environment variable with fallback.
    /// </summary>
    /// <param name="envVar">The environment variable name</param>
    /// <param name="defaultEndpoint">The default endpoint if env var is not set</param>
    /// <returns>A connected SpeculativeClient</returns>
    public static SpeculativeClient FromEnv(string envVar, string defaultEndpoint)
    {
        var endpoint = Environment.GetEnvironmentVariable(envVar);
        if (string.IsNullOrEmpty(endpoint))
        {
            endpoint = defaultEndpoint;
        }
        return Connect(endpoint);
    }

    /// <summary>
    /// Create a client from an existing channel.
    /// </summary>
    /// <param name="channel">The gRPC channel to use</param>
    /// <returns>A SpeculativeClient using the channel</returns>
    public static SpeculativeClient FromChannel(GrpcChannel channel)
    {
        return new SpeculativeClient(channel);
    }

    /// <summary>
    /// Execute a command speculatively against temporal aggregate state.
    /// </summary>
    /// <param name="request">The speculative aggregate request</param>
    /// <returns>The command response (without persistence)</returns>
    /// <exception cref="GrpcError">If the RPC fails</exception>
    public CommandResponse Aggregate(SpeculateAggregateRequest request)
    {
        try
        {
            return _aggregateStub.HandleSyncSpeculative(request);
        }
        catch (RpcException ex)
        {
            throw new GrpcError(ex.Message, ex.StatusCode);
        }
    }

    /// <summary>
    /// Execute a projector speculatively against events.
    /// </summary>
    /// <param name="request">The speculative projector request</param>
    /// <returns>The projection result (without persistence)</returns>
    /// <exception cref="GrpcError">If the RPC fails</exception>
    public Projection Projector(SpeculateProjectorRequest request)
    {
        try
        {
            return _projectorStub.HandleSpeculative(request);
        }
        catch (RpcException ex)
        {
            throw new GrpcError(ex.Message, ex.StatusCode);
        }
    }

    /// <summary>
    /// Execute a saga speculatively against events.
    /// </summary>
    /// <param name="request">The speculative saga request</param>
    /// <returns>The saga response (without persistence)</returns>
    /// <exception cref="GrpcError">If the RPC fails</exception>
    public SagaResponse Saga(SpeculateSagaRequest request)
    {
        try
        {
            return _sagaStub.ExecuteSpeculative(request);
        }
        catch (RpcException ex)
        {
            throw new GrpcError(ex.Message, ex.StatusCode);
        }
    }

    /// <summary>
    /// Execute a process manager speculatively.
    /// </summary>
    /// <param name="request">The speculative PM request</param>
    /// <returns>The PM response (without persistence)</returns>
    /// <exception cref="GrpcError">If the RPC fails</exception>
    public ProcessManagerHandleResponse ProcessManager(SpeculatePmRequest request)
    {
        try
        {
            return _pmStub.HandleSpeculative(request);
        }
        catch (RpcException ex)
        {
            throw new GrpcError(ex.Message, ex.StatusCode);
        }
    }

    /// <summary>
    /// Dispose the underlying channel.
    /// </summary>
    public void Dispose()
    {
        _channel?.Dispose();
    }
}
