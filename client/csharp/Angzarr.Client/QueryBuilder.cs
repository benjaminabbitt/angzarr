using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client;

/// <summary>
/// Fluent builder for constructing and executing event queries.
///
/// <para>QueryBuilder supports multiple access patterns:</para>
/// <list type="bullet">
///   <item>By root - fetch all events for a specific aggregate</item>
///   <item>By correlation ID - fetch events across aggregates in a workflow</item>
///   <item>By sequence range - fetch specific event windows for pagination</item>
///   <item>By temporal point - reconstruct historical state (as-of queries)</item>
///   <item>By edition - query from specific schema versions</item>
/// </list>
///
/// <example>
/// <code>
/// var events = client.Query("orders", orderId)
///     .Range(10)
///     .GetEventBook();
///
/// // Or temporal query
/// var historical = client.Query("orders", orderId)
///     .AsOfSequence(42)
///     .GetEventBook();
/// </code>
/// </example>
/// </summary>
public class QueryBuilder
{
    private readonly QueryClient _client;
    private readonly string _domain;
    private Guid? _root;
    private string? _correlationId;
    private Angzarr.SequenceRange? _rangeSelect;
    private Angzarr.TemporalQuery? _temporal;
    private string? _edition;
    private Exception? _error;

    /// <summary>
    /// Create a query builder for a specific aggregate.
    /// </summary>
    /// <param name="client">The query client to use</param>
    /// <param name="domain">The aggregate domain</param>
    /// <param name="root">The aggregate root GUID</param>
    /// <exception cref="InvalidArgumentError">If domain is empty</exception>
    public QueryBuilder(QueryClient client, string domain, Guid root)
    {
        if (string.IsNullOrEmpty(domain))
            throw new InvalidArgumentError("domain cannot be empty");
        _client = client;
        _domain = domain;
        _root = root;
    }

    /// <summary>
    /// Create a query builder by domain only (use with ByCorrelationId).
    /// </summary>
    /// <param name="client">The query client to use</param>
    /// <param name="domain">The aggregate domain</param>
    /// <exception cref="InvalidArgumentError">If domain is empty</exception>
    public QueryBuilder(QueryClient client, string domain)
    {
        if (string.IsNullOrEmpty(domain))
            throw new InvalidArgumentError("domain cannot be empty");
        _client = client;
        _domain = domain;
        _root = null;
    }

    /// <summary>
    /// Query by correlation ID instead of root.
    ///
    /// <para>Correlation IDs link events across aggregates in a distributed workflow.</para>
    /// </summary>
    /// <param name="id">The correlation ID</param>
    /// <returns>This builder for chaining</returns>
    public QueryBuilder ByCorrelationId(string id)
    {
        _correlationId = id;
        _root = null;
        return this;
    }

    /// <summary>
    /// Query events from a specific edition.
    ///
    /// <para>After upcasting (event schema migration), events exist in multiple editions.</para>
    /// </summary>
    /// <param name="edition">The edition name</param>
    /// <returns>This builder for chaining</returns>
    public QueryBuilder WithEdition(string edition)
    {
        _edition = edition;
        return this;
    }

    /// <summary>
    /// Query a range of sequences from lower (inclusive).
    ///
    /// <para>Use for incremental sync: "give me events since sequence N"</para>
    /// </summary>
    /// <param name="lower">The lower bound (inclusive)</param>
    /// <returns>This builder for chaining</returns>
    public QueryBuilder Range(int lower)
    {
        _rangeSelect = new Angzarr.SequenceRange { Lower = (uint)lower };
        return this;
    }

    /// <summary>
    /// Query a range of sequences with upper bound (inclusive).
    ///
    /// <para>Use for pagination: fetch events 100-200, then 200-300.</para>
    /// </summary>
    /// <param name="lower">The lower bound (inclusive)</param>
    /// <param name="upper">The upper bound (inclusive)</param>
    /// <returns>This builder for chaining</returns>
    public QueryBuilder RangeTo(int lower, int upper)
    {
        _rangeSelect = new Angzarr.SequenceRange
        {
            Lower = (uint)lower,
            Upper = (uint)upper
        };
        return this;
    }

    /// <summary>
    /// Query state as of a specific sequence number.
    ///
    /// <para>Essential for debugging: "What was the state when this bug occurred?"</para>
    /// </summary>
    /// <param name="seq">The sequence number</param>
    /// <returns>This builder for chaining</returns>
    public QueryBuilder AsOfSequence(int seq)
    {
        _temporal = new Angzarr.TemporalQuery
        {
            AsOfSequence = (uint)seq
        };
        return this;
    }

    /// <summary>
    /// Query state as of a specific timestamp (RFC3339 format).
    ///
    /// <para>Example: "2024-01-15T10:30:00Z"</para>
    /// </summary>
    /// <param name="rfc3339">The timestamp in RFC3339 format</param>
    /// <returns>This builder for chaining</returns>
    public QueryBuilder AsOfTime(string rfc3339)
    {
        try
        {
            var ts = Helpers.ParseTimestamp(rfc3339);
            _temporal = new Angzarr.TemporalQuery { AsOfTime = ts };
        }
        catch (Exception e)
        {
            _error = e;
        }
        return this;
    }

    /// <summary>
    /// Build the Query without executing.
    /// </summary>
    /// <returns>The constructed Query</returns>
    /// <exception cref="InvalidTimestampError">If timestamp parsing failed</exception>
    public Angzarr.Query Build()
    {
        if (_error != null)
            throw _error;

        var cover = new Angzarr.Cover { Domain = _domain };

        if (!string.IsNullOrEmpty(_correlationId))
            cover.CorrelationId = _correlationId;

        if (_root.HasValue)
            cover.Root = Helpers.UuidToProto(_root.Value);

        if (!string.IsNullOrEmpty(_edition))
            cover.Edition = new Angzarr.Edition { Name = _edition };

        var query = new Angzarr.Query { Cover = cover };

        if (_rangeSelect != null)
            query.Range = _rangeSelect;
        else if (_temporal != null)
            query.Temporal = _temporal;

        return query;
    }

    /// <summary>
    /// Execute the query and return a single EventBook.
    /// </summary>
    /// <returns>The EventBook containing matching events</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public Angzarr.EventBook GetEventBook()
    {
        var query = Build();
        return _client.GetEventBook(query);
    }

    /// <summary>
    /// Execute the query and return all matching EventBooks.
    /// </summary>
    /// <returns>List of EventBooks</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public List<Angzarr.EventBook> GetEvents()
    {
        var query = Build();
        return _client.GetEvents(query);
    }

    /// <summary>
    /// Execute the query and return just the event pages.
    ///
    /// <para>Convenience method when you only need events, not metadata.</para>
    /// </summary>
    /// <returns>List of EventPages</returns>
    /// <exception cref="GrpcError">If the gRPC call fails</exception>
    public IList<Angzarr.EventPage> GetPages()
    {
        var book = GetEventBook();
        return book.Pages;
    }
}
