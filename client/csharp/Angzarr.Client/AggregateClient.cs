using Grpc.Core;
using Grpc.Net.Client;

namespace Angzarr.Client;

/// <summary>
/// Client for sending commands to aggregates through the coordinator.
///
/// <para>AggregateClient handles command routing, response parsing, and provides
/// multiple execution modes:</para>
/// <list type="bullet">
///   <item>Async (fire-and-forget) - for high-throughput scenarios</item>
///   <item>Sync - wait for persistence, receive resulting events</item>
///   <item>Speculative - what-if execution without persistence</item>
/// </list>
///
/// <example>
/// <code>
/// using var client = AggregateClient.Connect("http://localhost:1310");
/// var command = buildCommand();
/// var response = client.Handle(command);
/// </code>
/// </example>
/// </summary>
public sealed class AggregateClient : IDisposable
{
    private readonly Angzarr.AggregateCoordinatorService.AggregateCoordinatorServiceClient _stub;
    private readonly GrpcChannel? _channel;
    private readonly bool _ownsChannel;

    private AggregateClient(GrpcChannel? channel, Angzarr.AggregateCoordinatorService.AggregateCoordinatorServiceClient stub, bool ownsChannel)
    {
        _channel = channel;
        _stub = stub;
        _ownsChannel = ownsChannel;
    }

    /// <summary>
    /// Connect to an aggregate coordinator at the given endpoint.
    /// </summary>
    /// <param name="endpoint">The server endpoint (e.g., "http://localhost:1310")</param>
    /// <returns>A new AggregateClient</returns>
    /// <exception cref="ConnectionError">If connection fails</exception>
    public static AggregateClient Connect(string endpoint)
    {
        try
        {
            var channel = GrpcChannel.ForAddress(FormatEndpoint(endpoint));
            var stub = new Angzarr.AggregateCoordinatorService.AggregateCoordinatorServiceClient(channel);
            return new AggregateClient(channel, stub, true);
        }
        catch (Exception e)
        {
            throw new ConnectionError($"Failed to connect to {endpoint}", e);
        }
    }

    /// <summary>
    /// Connect using an environment variable with fallback.
    /// </summary>
    /// <param name="envVar">The environment variable name</param>
    /// <param name="defaultEndpoint">Fallback endpoint if env var is not set</param>
    /// <returns>A new AggregateClient</returns>
    public static AggregateClient FromEnv(string envVar, string defaultEndpoint)
    {
        var endpoint = Environment.GetEnvironmentVariable(envVar);
        if (string.IsNullOrEmpty(endpoint))
            endpoint = defaultEndpoint;
        return Connect(endpoint);
    }

    /// <summary>
    /// Create a client from an existing channel.
    /// </summary>
    /// <param name="channel">The gRPC channel to use</param>
    /// <returns>A new AggregateClient that does not own the channel</returns>
    public static AggregateClient FromChannel(GrpcChannel channel)
    {
        var stub = new Angzarr.AggregateCoordinatorService.AggregateCoordinatorServiceClient(channel);
        return new AggregateClient(channel, stub, false);
    }

    /// <summary>
    /// Execute a command asynchronously (fire-and-forget).
    ///
    /// <para>Returns immediately after the coordinator accepts the command.
    /// The command is guaranteed to be processed, but the client doesn't wait.</para>
    /// </summary>
    /// <param name="command">The command to execute</param>
    /// <returns>The command response</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public Angzarr.CommandResponse Handle(Angzarr.CommandBook command)
    {
        try
        {
            return _stub.Handle(command);
        }
        catch (RpcException e)
        {
            throw new GrpcError(e.Message, e.StatusCode);
        }
    }

    /// <summary>
    /// Execute a command synchronously.
    ///
    /// <para>Blocks until the aggregate processes the command and events are persisted.
    /// The response includes the resulting events.</para>
    /// </summary>
    /// <param name="command">The sync command to execute</param>
    /// <returns>The command response with resulting events</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public Angzarr.CommandResponse HandleSync(Angzarr.SyncCommandBook command)
    {
        try
        {
            return _stub.HandleSync(command);
        }
        catch (RpcException e)
        {
            throw new GrpcError(e.Message, e.StatusCode);
        }
    }

    /// <summary>
    /// Execute a command speculatively against temporal state (no persistence).
    ///
    /// <para>Use for form validation, preview, or testing without polluting event store.</para>
    /// </summary>
    /// <param name="request">The speculative execution request</param>
    /// <returns>The command response with projected events</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public Angzarr.CommandResponse HandleSyncSpeculative(Angzarr.SpeculateAggregateRequest request)
    {
        try
        {
            return _stub.HandleSyncSpeculative(request);
        }
        catch (RpcException e)
        {
            throw new GrpcError(e.Message, e.StatusCode);
        }
    }

    /// <summary>
    /// Start building a command for the given domain and root.
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <param name="root">The aggregate root GUID</param>
    /// <returns>A CommandBuilder for fluent construction</returns>
    public CommandBuilder Command(string domain, Guid root)
    {
        return new CommandBuilder(this, domain, root);
    }

    /// <summary>
    /// Start building a command for a new aggregate (no root yet).
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <returns>A CommandBuilder for fluent construction</returns>
    public CommandBuilder CommandNew(string domain)
    {
        return new CommandBuilder(this, domain);
    }

    /// <summary>
    /// Close the underlying channel if this client owns it.
    /// </summary>
    public void Dispose()
    {
        if (_ownsChannel && _channel != null)
        {
            _channel.Dispose();
        }
    }

    private static string FormatEndpoint(string endpoint)
    {
        if (!endpoint.StartsWith("http://") && !endpoint.StartsWith("https://"))
        {
            return "http://" + endpoint;
        }
        return endpoint;
    }
}
