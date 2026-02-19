using Grpc.Core;
using Grpc.Net.Client;

namespace Angzarr.Client;

/// <summary>
/// Client for querying events from the EventQueryService.
///
/// <para>QueryClient provides read access to aggregate event streams for:</para>
/// <list type="bullet">
///   <item>State reconstruction - rebuild aggregate state from events</item>
///   <item>Audit trails - read complete history for compliance</item>
///   <item>Projections - feed events to read-model projectors</item>
///   <item>Testing - verify events were persisted after commands</item>
/// </list>
///
/// <example>
/// <code>
/// using var client = QueryClient.Connect("http://localhost:1340");
/// var query = new Query { Cover = new Cover { Domain = "test" } };
/// var events = client.GetEventBook(query);
/// </code>
/// </example>
/// </summary>
public sealed class QueryClient : IDisposable
{
    private readonly Angzarr.EventQueryService.EventQueryServiceClient _stub;
    private readonly GrpcChannel? _channel;
    private readonly bool _ownsChannel;

    private QueryClient(GrpcChannel? channel, Angzarr.EventQueryService.EventQueryServiceClient stub, bool ownsChannel)
    {
        _channel = channel;
        _stub = stub;
        _ownsChannel = ownsChannel;
    }

    /// <summary>
    /// Connect to an event query service at the given endpoint.
    /// </summary>
    /// <param name="endpoint">The server endpoint (e.g., "http://localhost:1340")</param>
    /// <returns>A new QueryClient</returns>
    /// <exception cref="ConnectionError">If connection fails</exception>
    public static QueryClient Connect(string endpoint)
    {
        try
        {
            var channel = GrpcChannel.ForAddress(FormatEndpoint(endpoint));
            var stub = new Angzarr.EventQueryService.EventQueryServiceClient(channel);
            return new QueryClient(channel, stub, true);
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
    /// <returns>A new QueryClient</returns>
    public static QueryClient FromEnv(string envVar, string defaultEndpoint)
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
    /// <returns>A new QueryClient that does not own the channel</returns>
    public static QueryClient FromChannel(GrpcChannel channel)
    {
        var stub = new Angzarr.EventQueryService.EventQueryServiceClient(channel);
        return new QueryClient(channel, stub, false);
    }

    /// <summary>
    /// Retrieve a single EventBook for the query.
    /// </summary>
    /// <param name="query">The query specifying which events to retrieve</param>
    /// <returns>The EventBook containing matching events</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public Angzarr.EventBook GetEventBook(Angzarr.Query query)
    {
        try
        {
            return _stub.GetEventBook(query);
        }
        catch (RpcException e)
        {
            throw new GrpcError(e.Message, e.StatusCode);
        }
    }

    /// <summary>
    /// Retrieve all EventBooks matching the query.
    /// </summary>
    /// <param name="query">The query specifying which events to retrieve</param>
    /// <returns>List of EventBooks</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public List<Angzarr.EventBook> GetEvents(Angzarr.Query query)
    {
        try
        {
            var events = new List<Angzarr.EventBook>();
            using var call = _stub.GetEvents(query);
            while (call.ResponseStream.MoveNext(default).Result)
            {
                events.Add(call.ResponseStream.Current);
            }
            return events;
        }
        catch (RpcException e)
        {
            throw new GrpcError(e.Message, e.StatusCode);
        }
    }

    /// <summary>
    /// Start building a query for the given domain and root.
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <param name="root">The aggregate root GUID</param>
    /// <returns>A QueryBuilder for fluent construction</returns>
    public QueryBuilder Query(string domain, Guid root)
    {
        return new QueryBuilder(this, domain, root);
    }

    /// <summary>
    /// Start building a query by domain only (use with ByCorrelationId).
    /// </summary>
    /// <param name="domain">The aggregate domain</param>
    /// <returns>A QueryBuilder for fluent construction</returns>
    public QueryBuilder QueryDomain(string domain)
    {
        return new QueryBuilder(this, domain);
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
        // Ensure the endpoint has a scheme
        if (!endpoint.StartsWith("http://") && !endpoint.StartsWith("https://"))
        {
            return "http://" + endpoint;
        }
        return endpoint;
    }
}
