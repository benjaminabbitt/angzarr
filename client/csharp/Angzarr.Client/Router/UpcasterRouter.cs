using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client;

/// <summary>
/// Event version transformer.
///
/// Matches old event type_url suffixes and transforms to new versions.
/// Events without registered transformations pass through unchanged.
///
/// Example:
/// <code>
/// var router = new UpcasterRouter("player")
///     .On("PlayerRegisteredV1", old => {
///         var v1 = old.Unpack&lt;PlayerRegisteredV1&gt;();
///         return Any.Pack(new PlayerRegistered {
///             DisplayName = v1.DisplayName,
///             Email = v1.Email,
///             AiModelId = ""  // New field with default
///         }, "type.googleapis.com/");
///     });
///
/// var newEvents = router.Upcast(oldEvents);
/// </code>
/// </summary>
public class UpcasterRouter
{
    private readonly string _domain;
    private readonly List<UpcasterEntry> _handlers = new();

    private sealed class UpcasterEntry
    {
        public string Suffix { get; }
        public Func<Any, Any> Handler { get; }

        public UpcasterEntry(string suffix, Func<Any, Any> handler)
        {
            Suffix = suffix;
            Handler = handler;
        }
    }

    /// <summary>
    /// Create a new upcaster router for a domain.
    /// </summary>
    /// <param name="domain">The domain this upcaster handles.</param>
    public UpcasterRouter(string domain)
    {
        _domain = domain;
    }

    /// <summary>
    /// Get the domain this upcaster handles.
    /// </summary>
    public string Domain => _domain;

    /// <summary>
    /// Register a handler for an old event type_url suffix.
    ///
    /// The suffix is matched against the end of the event's type_url.
    /// For example, suffix "PlayerRegisteredV1" matches
    /// "type.googleapis.com/examples.PlayerRegisteredV1".
    /// </summary>
    /// <param name="suffix">The type_url suffix to match.</param>
    /// <param name="handler">Function that transforms old event to new event.</param>
    /// <returns>This router for fluent chaining.</returns>
    public UpcasterRouter On(string suffix, Func<Any, Any> handler)
    {
        _handlers.Add(new UpcasterEntry(suffix, handler));
        return this;
    }

    /// <summary>
    /// Transform a list of events to current versions.
    ///
    /// Events matching registered handlers are transformed.
    /// Events without matching handlers pass through unchanged.
    /// </summary>
    /// <param name="events">List of EventPages to transform.</param>
    /// <returns>List of EventPages with transformed events.</returns>
    public IReadOnlyList<Angzarr.EventPage> Upcast(IEnumerable<Angzarr.EventPage> events)
    {
        var result = new List<Angzarr.EventPage>();

        foreach (var page in events)
        {
            if (page.Event == null)
            {
                result.Add(page);
                continue;
            }

            var eventAny = page.Event;
            var typeUrl = eventAny.TypeUrl;
            var transformed = false;

            foreach (var entry in _handlers)
            {
                if (typeUrl.EndsWith(entry.Suffix))
                {
                    var newEvent = entry.Handler(eventAny);
                    var newPage = new Angzarr.EventPage
                    {
                        Event = newEvent,
                        Header = page.Header?.Clone() ?? new Angzarr.PageHeader(),
                        CreatedAt = page.CreatedAt,
                    };
                    result.Add(newPage);
                    transformed = true;
                    break;
                }
            }

            if (!transformed)
            {
                result.Add(page);
            }
        }

        return result;
    }
}
