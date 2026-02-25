using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client.Router;

/// <summary>
/// Router for process manager components (events -> commands + PM events, multi-domain).
///
/// Domains are registered via fluent Domain() calls.
///
/// Example:
/// <code>
/// var router = new ProcessManagerRouter&lt;HandFlowState&gt;(
///     "pmg-hand-flow",
///     "hand-flow",
///     events => RebuildState(events)
/// )
///     .Domain("order", new OrderPmHandler())
///     .Domain("inventory", new InventoryPmHandler());
///
/// // Phase 1: Get destinations needed
/// var destinations = router.PrepareDestinations(trigger, processState);
///
/// // Phase 2: Execute with fetched destinations
/// var response = router.Dispatch(trigger, processState, fetchedDestinations);
/// </code>
/// </summary>
/// <typeparam name="TState">The PM state type.</typeparam>
public class ProcessManagerRouter<TState>
    where TState : new()
{
    private readonly string _name;
    private readonly string _pmDomain;
    private readonly Func<Angzarr.EventBook?, TState> _rebuild;
    private readonly Dictionary<string, IProcessManagerDomainHandler<TState>> _domains = new();

    /// <summary>
    /// Create a new process manager router.
    /// Process managers correlate events across multiple domains and maintain
    /// their own state. The pmDomain is used for storing PM state.
    /// </summary>
    /// <param name="name">Component name.</param>
    /// <param name="pmDomain">Domain for PM's own state.</param>
    /// <param name="rebuild">Function to rebuild state from events.</param>
    public ProcessManagerRouter(
        string name,
        string pmDomain,
        Func<Angzarr.EventBook?, TState> rebuild
    )
    {
        _name = name;
        _pmDomain = pmDomain;
        _rebuild = rebuild;
    }

    /// <summary>
    /// Register a domain handler.
    /// Process managers can have multiple input domains.
    /// </summary>
    /// <param name="name">Domain name.</param>
    /// <param name="handler">Handler for events from this domain.</param>
    /// <returns>This router for fluent chaining.</returns>
    public ProcessManagerRouter<TState> Domain<THandler>(string name, THandler handler)
        where THandler : IProcessManagerDomainHandler<TState>
    {
        _domains[name] = handler;
        return this;
    }

    /// <summary>
    /// Get the router name.
    /// </summary>
    public string Name => _name;

    /// <summary>
    /// Get the PM's own domain (for state storage).
    /// </summary>
    public string PmDomain => _pmDomain;

    /// <summary>
    /// Get subscriptions (domain + event types) for this PM.
    /// Returns list of (domain, event types) tuples.
    /// </summary>
    public IReadOnlyList<(string Domain, IReadOnlyList<string> Types)> Subscriptions()
    {
        return _domains
            .Select(kv => (kv.Key, (IReadOnlyList<string>)kv.Value.EventTypes()))
            .ToList();
    }

    /// <summary>
    /// Rebuild PM state from events.
    /// </summary>
    public TState RebuildState(Angzarr.EventBook? events)
    {
        return _rebuild(events);
    }

    /// <summary>
    /// Get destinations needed for the given trigger and process state.
    /// </summary>
    /// <param name="trigger">The triggering event book, may be null.</param>
    /// <param name="processState">Current PM state as event book, may be null.</param>
    /// <returns>List of covers identifying needed destination aggregates.</returns>
    public IReadOnlyList<Angzarr.Cover> PrepareDestinations(
        Angzarr.EventBook? trigger,
        Angzarr.EventBook? processState
    )
    {
        if (trigger == null || trigger.Pages.Count == 0)
            return Array.Empty<Angzarr.Cover>();

        var triggerDomain = trigger.Cover?.Domain ?? "";

        var eventPage = trigger.Pages[^1]; // Last page
        var eventAny = eventPage.Event;
        if (eventAny == null)
            return Array.Empty<Angzarr.Cover>();

        var state = processState != null ? RebuildState(processState) : new TState();

        if (!_domains.TryGetValue(triggerDomain, out var handler))
            return Array.Empty<Angzarr.Cover>();

        return handler.Prepare(trigger, state, eventAny);
    }

    /// <summary>
    /// Dispatch a trigger event to the appropriate handler.
    /// </summary>
    /// <param name="trigger">The triggering event book.</param>
    /// <param name="processState">Current PM state as event book.</param>
    /// <param name="destinations">Fetched destination aggregate states.</param>
    /// <returns>PM handle response with commands and process events.</returns>
    public Angzarr.ProcessManagerHandleResponse Dispatch(
        Angzarr.EventBook trigger,
        Angzarr.EventBook processState,
        IReadOnlyList<Angzarr.EventBook>? destinations = null
    )
    {
        var triggerDomain = trigger.Cover?.Domain ?? "";

        if (!_domains.TryGetValue(triggerDomain, out var handler))
            throw new InvalidArgumentError($"No handler for domain: {triggerDomain}");

        if (trigger.Pages.Count == 0)
            throw new InvalidArgumentError("Trigger event book has no events");

        var eventPage = trigger.Pages[^1]; // Last page
        var eventAny = eventPage.Event;
        if (eventAny == null)
            throw new InvalidArgumentError("Missing event payload");

        var state = RebuildState(processState);

        // Check for Notification
        if (eventAny.TypeUrl.EndsWith("Notification"))
        {
            return DispatchNotification(handler, eventAny, state);
        }

        var response = handler.Handle(
            trigger,
            state,
            eventAny,
            destinations ?? Array.Empty<Angzarr.EventBook>()
        );

        var result = new Angzarr.ProcessManagerHandleResponse();
        result.Commands.AddRange(response.Commands);
        result.ProcessEvents = response.ProcessEvents;
        return result;
    }

    private Angzarr.ProcessManagerHandleResponse DispatchNotification(
        IProcessManagerDomainHandler<TState> handler,
        Any eventAny,
        TState state
    )
    {
        var notification = eventAny.Unpack<Angzarr.Notification>();

        var rejection = notification.Payload?.Unpack<Angzarr.RejectionNotification>();
        var domain = "";
        var commandSuffix = "";

        if (rejection?.RejectedCommand?.Pages.Count > 0)
        {
            var rejectedCmd = rejection.RejectedCommand;
            domain = rejectedCmd.Cover?.Domain ?? "";
            var cmdTypeUrl = rejectedCmd.Pages[0].Command?.TypeUrl ?? "";
            commandSuffix = Helpers.TypeNameFromUrl(cmdTypeUrl);
        }

        var response = handler.OnRejected(notification, state, domain, commandSuffix);

        return new Angzarr.ProcessManagerHandleResponse { ProcessEvents = response.Events };
    }
}
