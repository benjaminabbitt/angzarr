using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client;

/// <summary>
/// Fluent builder for constructing and executing commands.
///
/// <para>CommandBuilder reduces boilerplate when creating commands:</para>
/// <list type="bullet">
///   <item>Chain method calls instead of nested object construction</item>
///   <item>Type-safe methods prevent invalid field combinations</item>
///   <item>Auto-generates correlation IDs when not provided</item>
///   <item>Build incrementally, execute when ready</item>
/// </list>
///
/// <example>
/// <code>
/// var response = client.Command("orders", orderId)
///     .WithCorrelationId("corr-123")
///     .WithSequence(5)
///     .WithCommand(typeUrl, createOrderCmd)
///     .Execute();
/// </code>
/// </example>
/// </summary>
public class CommandBuilder
{
    private readonly AggregateClient _client;
    private readonly string _domain;
    private readonly Guid? _root;
    private string? _correlationId;
    private uint _sequence = 0;
    private string? _typeUrl;
    private byte[]? _payload;
    private Exception? _error;

    /// <summary>
    /// Create a command builder for an existing aggregate.
    /// </summary>
    /// <param name="client">The aggregate client to use</param>
    /// <param name="domain">The aggregate domain</param>
    /// <param name="root">The aggregate root GUID</param>
    public CommandBuilder(AggregateClient client, string domain, Guid root)
    {
        _client = client;
        _domain = domain;
        _root = root;
    }

    /// <summary>
    /// Create a command builder for a new aggregate (no root yet).
    /// </summary>
    /// <param name="client">The aggregate client to use</param>
    /// <param name="domain">The aggregate domain</param>
    public CommandBuilder(AggregateClient client, string domain)
    {
        _client = client;
        _domain = domain;
        _root = null;
    }

    /// <summary>
    /// Set the correlation ID for request tracing.
    ///
    /// <para>Correlation IDs link related operations across services.
    /// If not set, a GUID will be auto-generated on build.</para>
    /// </summary>
    /// <param name="id">The correlation ID</param>
    /// <returns>This builder for chaining</returns>
    public CommandBuilder WithCorrelationId(string id)
    {
        _correlationId = id;
        return this;
    }

    /// <summary>
    /// Set the expected sequence number for optimistic locking.
    ///
    /// <para>Defaults to 0 for new aggregates.</para>
    /// </summary>
    /// <param name="seq">The sequence number</param>
    /// <returns>This builder for chaining</returns>
    public CommandBuilder WithSequence(int seq)
    {
        _sequence = (uint)seq;
        return this;
    }

    /// <summary>
    /// Set the command type URL and message.
    /// </summary>
    /// <param name="typeUrl">The fully-qualified type URL (e.g., "type.googleapis.com/orders.CreateOrder")</param>
    /// <param name="message">The protobuf command message</param>
    /// <returns>This builder for chaining</returns>
    public CommandBuilder WithCommand(string typeUrl, IMessage message)
    {
        try
        {
            _typeUrl = typeUrl;
            _payload = message.ToByteArray();
        }
        catch (Exception e)
        {
            _error = new InvalidArgumentError($"Failed to serialize command: {e.Message}");
        }
        return this;
    }

    /// <summary>
    /// Build the CommandBook without executing.
    /// </summary>
    /// <returns>The constructed CommandBook</returns>
    /// <exception cref="InvalidArgumentError">If required fields are missing</exception>
    public Angzarr.CommandBook Build()
    {
        if (_error != null)
            throw _error;

        if (string.IsNullOrEmpty(_typeUrl))
            throw new InvalidArgumentError("command type_url not set");

        if (_payload == null)
            throw new InvalidArgumentError("command payload not set");

        var correlationId = _correlationId;
        if (string.IsNullOrEmpty(correlationId))
            correlationId = Guid.NewGuid().ToString();

        var cover = new Angzarr.Cover
        {
            Domain = _domain,
            CorrelationId = correlationId
        };

        if (_root.HasValue)
            cover.Root = Helpers.UuidToProto(_root.Value);

        var commandAny = new Any
        {
            TypeUrl = _typeUrl,
            Value = ByteString.CopyFrom(_payload)
        };

        var page = new Angzarr.CommandPage
        {
            Sequence = _sequence,
            Command = commandAny
        };

        var book = new Angzarr.CommandBook { Cover = cover };
        book.Pages.Add(page);

        return book;
    }

    /// <summary>
    /// Build and execute the command.
    /// </summary>
    /// <returns>The command response</returns>
    /// <exception cref="InvalidArgumentError">If required fields are missing</exception>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public Angzarr.CommandResponse Execute()
    {
        var cmd = Build();
        return _client.Handle(cmd);
    }
}
