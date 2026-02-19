using Grpc.Net.Client;

namespace Angzarr.Client;

/// <summary>
/// Combined client for aggregate commands and event queries.
///
/// <para>DomainClient combines QueryClient and AggregateClient into a single unified
/// interface. This is the recommended entry point for most applications because:</para>
/// <list type="bullet">
///   <item>Single connection - one endpoint, one channel, reduced resource usage</item>
///   <item>Unified API - both queries and commands through one object</item>
///   <item>Builder access - fluent builders attached to the client instance</item>
///   <item>Simpler DI - inject one client instead of two</item>
/// </list>
///
/// <para>For advanced use cases (separate scaling, different endpoints), use
/// QueryClient and AggregateClient directly.</para>
///
/// <example>
/// <code>
/// using var client = DomainClient.Connect("http://localhost:1310");
///
/// // Send a command
/// var response = client.Command("orders", orderId)
///     .WithCommand(typeUrl, createOrderCmd)
///     .Execute();
///
/// // Query events
/// var events = client.Query("orders", orderId).GetEventBook();
/// </code>
/// </example>
/// </summary>
public sealed class DomainClient : IDisposable
{
    private readonly AggregateClient _aggregate;
    private readonly QueryClient _query;
    private readonly GrpcChannel? _channel;

    private DomainClient(GrpcChannel? channel, AggregateClient aggregate, QueryClient query)
    {
        _channel = channel;
        _aggregate = aggregate;
        _query = query;
    }

    /// <summary>
    /// Connect to a domain's coordinator at the given endpoint.
    /// </summary>
    /// <param name="endpoint">The server endpoint (e.g., "http://localhost:1310")</param>
    /// <returns>A new DomainClient</returns>
    /// <exception cref="ConnectionError">If connection fails</exception>
    public static DomainClient Connect(string endpoint)
    {
        try
        {
            var formattedEndpoint = FormatEndpoint(endpoint);
            var channel = GrpcChannel.ForAddress(formattedEndpoint);
            return new DomainClient(
                channel,
                AggregateClient.FromChannel(channel),
                QueryClient.FromChannel(channel)
            );
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
    /// <returns>A new DomainClient</returns>
    public static DomainClient FromEnv(string envVar, string defaultEndpoint)
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
    /// <returns>A new DomainClient</returns>
    public static DomainClient FromChannel(GrpcChannel channel)
    {
        return new DomainClient(
            null, // Don't own the channel
            AggregateClient.FromChannel(channel),
            QueryClient.FromChannel(channel)
        );
    }

    /// <summary>
    /// Get the aggregate client for direct access.
    /// </summary>
    public AggregateClient Aggregate => _aggregate;

    /// <summary>
    /// Get the query client for direct access.
    /// </summary>
    public QueryClient Query => _query;

    /// <summary>
    /// Execute a command (convenience method delegating to aggregate).
    /// </summary>
    /// <param name="command">The command to execute</param>
    /// <returns>The command response</returns>
    public Angzarr.CommandResponse Execute(Angzarr.CommandBook command)
    {
        return _aggregate.Handle(command);
    }

    /// <summary>
    /// Start building a command for the given domain and root.
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <param name="root">The aggregate root GUID</param>
    /// <returns>A CommandBuilder for fluent construction</returns>
    public CommandBuilder Command(string domain, Guid root)
    {
        return _aggregate.Command(domain, root);
    }

    /// <summary>
    /// Start building a command for a new aggregate (no root yet).
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <returns>A CommandBuilder for fluent construction</returns>
    public CommandBuilder CommandNew(string domain)
    {
        return _aggregate.CommandNew(domain);
    }

    /// <summary>
    /// Start building a query for the given domain and root.
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <param name="root">The aggregate root GUID</param>
    /// <returns>A QueryBuilder for fluent construction</returns>
    public QueryBuilder QueryEvents(string domain, Guid root)
    {
        return _query.Query(domain, root);
    }

    /// <summary>
    /// Start building a query by domain only (use with ByCorrelationId).
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <returns>A QueryBuilder for fluent construction</returns>
    public QueryBuilder QueryDomain(string domain)
    {
        return _query.QueryDomain(domain);
    }

    /// <summary>
    /// Close the underlying channel and clients.
    /// </summary>
    public void Dispose()
    {
        _aggregate.Dispose();
        _query.Dispose();

        if (_channel != null)
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
