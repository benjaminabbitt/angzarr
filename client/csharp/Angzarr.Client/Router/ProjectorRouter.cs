namespace Angzarr.Client.Router;

/// <summary>
/// Router for projector components (events -> external output, multi-domain).
///
/// Domains are registered via fluent Domain() calls.
///
/// Example:
/// <code>
/// var router = new ProjectorRouter("prj-output")
///     .Domain("player", new PlayerProjectorHandler())
///     .Domain("hand", new HandProjectorHandler());
///
/// // Dispatch events
/// var projection = router.Dispatch(eventBook);
/// </code>
/// </summary>
public class ProjectorRouter
{
    private readonly string _name;
    private readonly Dictionary<string, IProjectorDomainHandler> _domains = new();

    /// <summary>
    /// Create a new projector router.
    /// Projectors consume events from multiple domains and produce external output.
    /// </summary>
    /// <param name="name">Component name.</param>
    public ProjectorRouter(string name)
    {
        _name = name;
    }

    /// <summary>
    /// Register a domain handler.
    /// Projectors can have multiple input domains.
    /// </summary>
    /// <param name="name">Domain name.</param>
    /// <param name="handler">Handler for events from this domain.</param>
    /// <returns>This router for fluent chaining.</returns>
    public ProjectorRouter Domain<THandler>(string name, THandler handler)
        where THandler : IProjectorDomainHandler
    {
        _domains[name] = handler;
        return this;
    }

    /// <summary>
    /// Get the router name.
    /// </summary>
    public string Name => _name;

    /// <summary>
    /// Get subscriptions (domain + event types) for this projector.
    /// Returns list of (domain, event types) tuples.
    /// </summary>
    public IReadOnlyList<(string Domain, IReadOnlyList<string> Types)> Subscriptions()
    {
        return _domains
            .Select(kv => (kv.Key, (IReadOnlyList<string>)kv.Value.EventTypes()))
            .ToList();
    }

    /// <summary>
    /// Dispatch events to the appropriate handler.
    /// </summary>
    /// <param name="events">Event book to process.</param>
    /// <returns>Projection result.</returns>
    public Angzarr.Projection Dispatch(Angzarr.EventBook events)
    {
        var domain = events.Cover?.Domain ?? "";

        if (!_domains.TryGetValue(domain, out var handler))
            throw new InvalidArgumentError($"No handler for domain: {domain}");

        return handler.Project(events);
    }
}
