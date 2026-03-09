using Google.Protobuf;

namespace Angzarr.Client;

/// <summary>
/// Context for saga handlers, providing access to destination aggregate state.
///
/// Used in the splitter pattern where one event triggers commands to multiple aggregates.
/// Provides sequence number lookup for optimistic concurrency control.
///
/// <code>
/// List&lt;CommandBook&gt; HandleTableSettled(TableSettled evt, SagaContext ctx)
/// {
///     var commands = new List&lt;CommandBook&gt;();
///     foreach (var payout in evt.Payouts)
///     {
///         var seq = ctx.GetSequence("player", payout.PlayerRoot);
///         var cmd = new TransferFunds { PlayerRoot = payout.PlayerRoot, Amount = payout.Amount };
///         commands.Add(NewCommandBook("player", cmd, seq));
///     }
///     return commands;
/// }
/// </code>
/// </summary>
public class SagaContext
{
    private readonly Dictionary<string, Angzarr.EventBook> _destinations;

    /// <summary>
    /// Create a context from a list of destination EventBooks.
    /// </summary>
    /// <param name="destinationBooks">List of EventBooks fetched during prepare phase.</param>
    public SagaContext(IEnumerable<Angzarr.EventBook> destinationBooks)
    {
        _destinations = new Dictionary<string, Angzarr.EventBook>();
        foreach (var book in destinationBooks)
        {
            if (book.Cover != null && !string.IsNullOrEmpty(book.Cover.Domain))
            {
                var key = MakeKey(book.Cover.Domain, book.Cover.Root?.Value ?? ByteString.Empty);
                _destinations[key] = book;
            }
        }
    }

    /// <summary>
    /// Get the next sequence number for a destination aggregate.
    /// Returns 1 if the aggregate doesn't exist yet.
    /// </summary>
    /// <param name="domain">The domain of the target aggregate.</param>
    /// <param name="aggregateRoot">The root identifier.</param>
    /// <returns>The next sequence number for the aggregate.</returns>
    public uint GetSequence(string domain, ByteString aggregateRoot)
    {
        var key = MakeKey(domain, aggregateRoot);
        if (_destinations.TryGetValue(key, out var book))
        {
            if (book.Pages.Count == 0)
                return 1;
            var lastPage = book.Pages[^1];
            return (lastPage.Header?.Sequence ?? 0) + 1;
        }
        return 1;
    }

    /// <summary>
    /// Get the next sequence number for a destination aggregate.
    /// Returns 1 if the aggregate doesn't exist yet.
    /// </summary>
    /// <param name="domain">The domain of the target aggregate.</param>
    /// <param name="aggregateRoot">The root identifier as bytes.</param>
    /// <returns>The next sequence number for the aggregate.</returns>
    public uint GetSequence(string domain, byte[] aggregateRoot)
    {
        return GetSequence(domain, ByteString.CopyFrom(aggregateRoot));
    }

    /// <summary>
    /// Get the EventBook for a destination aggregate, if available.
    /// </summary>
    /// <param name="domain">The domain of the target aggregate.</param>
    /// <param name="aggregateRoot">The root identifier.</param>
    /// <returns>The EventBook if found, null otherwise.</returns>
    public Angzarr.EventBook? GetDestination(string domain, ByteString aggregateRoot)
    {
        var key = MakeKey(domain, aggregateRoot);
        return _destinations.TryGetValue(key, out var book) ? book : null;
    }

    /// <summary>
    /// Get the EventBook for a destination aggregate, if available.
    /// </summary>
    /// <param name="domain">The domain of the target aggregate.</param>
    /// <param name="aggregateRoot">The root identifier as bytes.</param>
    /// <returns>The EventBook if found, null otherwise.</returns>
    public Angzarr.EventBook? GetDestination(string domain, byte[] aggregateRoot)
    {
        return GetDestination(domain, ByteString.CopyFrom(aggregateRoot));
    }

    /// <summary>
    /// Check if a destination exists.
    /// </summary>
    /// <param name="domain">The domain of the target aggregate.</param>
    /// <param name="aggregateRoot">The root identifier.</param>
    /// <returns>True if the destination exists.</returns>
    public bool HasDestination(string domain, ByteString aggregateRoot)
    {
        var key = MakeKey(domain, aggregateRoot);
        return _destinations.ContainsKey(key);
    }

    private static string MakeKey(string domain, ByteString root)
    {
        return $"{domain}:{root.ToBase64()}";
    }
}
