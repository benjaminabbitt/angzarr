using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Error message constants.
/// </summary>
public static class ErrorMessages
{
    public const string UnknownCommand = "Unknown command type";
    public const string NoCommandPages = "No command pages";
}

/// <summary>
/// Delegate for command handlers in the functional pattern.
/// </summary>
public delegate Angzarr.EventBook CommandHandler<TState>(
    Angzarr.CommandBook commandBook,
    Any commandAny,
    TState state,
    int seq);

/// <summary>
/// Delegate for state rebuild functions.
/// </summary>
public delegate TState StateRebuilder<TState>(Angzarr.EventBook? eventBook);

/// <summary>
/// Response from rejection handlers - can emit events AND/OR notification.
/// </summary>
public class RejectionHandlerResponse
{
    /// <summary>
    /// Events to persist to own state (compensation).
    /// </summary>
    public Angzarr.EventBook? Events { get; set; }

    /// <summary>
    /// Notification to forward upstream (rejection propagation).
    /// </summary>
    public Angzarr.Notification? Notification { get; set; }
}

/// <summary>
/// Delegate for rejection handlers.
/// </summary>
public delegate RejectionHandlerResponse RejectionHandler<TState>(
    Angzarr.Notification notification,
    TState state);

/// <summary>
/// DRY command dispatcher for aggregates.
/// Matches command type_url suffixes and dispatches to registered handlers.
/// </summary>
public class CommandRouter<TState> where TState : new()
{
    private readonly string _domain;
    private StateRebuilder<TState>? _rebuild;
    private StateRouter<TState>? _stateRouter;
    private readonly List<(string Suffix, CommandHandler<TState> Handler)> _handlers = new();
    private readonly Dictionary<string, RejectionHandler<TState>> _rejectionHandlers = new();

    public CommandRouter(string domain, StateRebuilder<TState>? rebuild = null)
    {
        _domain = domain;
        _rebuild = rebuild;
    }

    /// <summary>
    /// Compose a StateRouter for state reconstruction.
    /// </summary>
    public CommandRouter<TState> WithState(StateRouter<TState> stateRouter)
    {
        _stateRouter = stateRouter;
        return this;
    }

    /// <summary>
    /// Register a handler for a command type_url suffix.
    /// </summary>
    public CommandRouter<TState> On(string suffix, CommandHandler<TState> handler)
    {
        _handlers.Add((suffix, handler));
        return this;
    }

    /// <summary>
    /// Register a rejection handler for compensation.
    /// </summary>
    public CommandRouter<TState> OnRejected(string domain, string command, RejectionHandler<TState> handler)
    {
        _rejectionHandlers[$"{domain}/{command}"] = handler;
        return this;
    }

    /// <summary>
    /// Dispatch a ContextualCommand to the matching handler.
    /// </summary>
    public Angzarr.BusinessResponse Dispatch(Angzarr.ContextualCommand cmd)
    {
        var commandBook = cmd.Command;
        var priorEvents = cmd.Events;

        var state = GetState(priorEvents);
        var seq = Helpers.NextSequence(priorEvents);

        if (commandBook.Pages.Count == 0)
            throw new InvalidArgumentError(ErrorMessages.NoCommandPages);

        var commandAny = commandBook.Pages[0].Command;
        if (string.IsNullOrEmpty(commandAny?.TypeUrl))
            throw new InvalidArgumentError(ErrorMessages.NoCommandPages);

        var typeUrl = commandAny.TypeUrl;

        // Check for Notification (rejection/compensation)
        if (typeUrl.EndsWith("Notification"))
        {
            var notification = commandAny.Unpack<Angzarr.Notification>();
            return DispatchRejection(notification, state);
        }

        // Normal command dispatch
        foreach (var (suffix, handler) in _handlers)
        {
            if (typeUrl.EndsWith(suffix))
            {
                var events = handler(commandBook, commandAny, state, seq);
                return new Angzarr.BusinessResponse { Events = events };
            }
        }

        throw new InvalidArgumentError($"{ErrorMessages.UnknownCommand}: {typeUrl}");
    }

    private Angzarr.BusinessResponse DispatchRejection(Angzarr.Notification notification, TState state)
    {
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

        foreach (var (key, handler) in _rejectionHandlers)
        {
            var parts = key.Split('/');
            if (parts[0] == domain && commandSuffix.EndsWith(parts[1]))
            {
                var response = handler(notification, state);
                // Handle notification forwarding
                if (response.Notification != null)
                {
                    return new Angzarr.BusinessResponse { Notification = response.Notification };
                }
                // Handle compensation events
                if (response.Events != null)
                {
                    return new Angzarr.BusinessResponse { Events = response.Events };
                }
                // Handler returned empty response
                return new Angzarr.BusinessResponse
                {
                    Revocation = new Angzarr.RevocationResponse
                    {
                        EmitSystemRevocation = false,
                        Reason = $"Aggregate {_domain} handled rejection for {key}"
                    }
                };
            }
        }

        return new Angzarr.BusinessResponse
        {
            Revocation = new Angzarr.RevocationResponse
            {
                EmitSystemRevocation = true,
                Reason = $"Aggregate {_domain} has no custom compensation for {domain}/{commandSuffix}"
            }
        };
    }

    private TState GetState(Angzarr.EventBook? eventBook)
    {
        if (_stateRouter != null)
            return _stateRouter.WithEventBook(eventBook);
        if (_rebuild != null)
            return _rebuild(eventBook);
        throw new InvalidOperationException(
            "CommandRouter requires either rebuild function or StateRouter via WithState()");
    }
}

/// <summary>
/// Delegate for event handlers in the functional pattern.
/// </summary>
public delegate List<Angzarr.CommandBook> EventHandler(
    Any eventAny,
    byte[]? root,
    string correlationId,
    List<Angzarr.EventBook>? destinations);

/// <summary>
/// Delegate for prepare handlers in two-phase protocol.
/// </summary>
public delegate List<Angzarr.Cover> PrepareHandler(Any eventAny, Angzarr.UUID? root);

/// <summary>
/// Unified event dispatcher for sagas, process managers, and projectors.
/// Uses fluent .Domain().On() pattern to register handlers with domain context.
///
/// Example (Saga - single domain):
/// <code>
/// var router = new EventRouter("saga-table-hand")
///     .Domain("table")
///     .On("HandStarted", HandleStarted);
/// </code>
///
/// Example (Process Manager - multi-domain):
/// <code>
/// var router = new EventRouter("pmg-order-flow")
///     .Domain("order")
///     .On("OrderCreated", HandleCreated)
///     .Domain("inventory")
///     .On("StockReserved", HandleReserved);
/// </code>
///
/// Example (Projector - multi-domain):
/// <code>
/// var router = new EventRouter("prj-output")
///     .Domain("player")
///     .On("PlayerRegistered", HandleRegistered)
///     .Domain("hand")
///     .On("CardsDealt", HandleDealt);
/// </code>
/// </summary>
public class EventRouter
{
    private readonly string _name;
    private string? _currentDomain;
    private readonly Dictionary<string, List<(string Suffix, EventHandler Handler)>> _handlers = new();
    private readonly Dictionary<string, Dictionary<string, PrepareHandler>> _prepareHandlers = new();

    public EventRouter(string name)
    {
        _name = name;
    }

    /// <summary>
    /// Create a new EventRouter with a single input domain (backwards compatibility).
    /// </summary>
    /// <param name="name">Component name</param>
    /// <param name="inputDomain">Single input domain (deprecated, use Domain() instead)</param>
    [Obsolete("Use new EventRouter(name).Domain(inputDomain) instead")]
    public EventRouter(string name, string inputDomain) : this(name)
    {
        if (!string.IsNullOrEmpty(inputDomain))
            Domain(inputDomain);
    }

    /// <summary>
    /// Set the current domain context for subsequent On() calls.
    /// </summary>
    public EventRouter Domain(string name)
    {
        _currentDomain = name;
        if (!_handlers.ContainsKey(name))
            _handlers[name] = new List<(string, EventHandler)>();
        if (!_prepareHandlers.ContainsKey(name))
            _prepareHandlers[name] = new Dictionary<string, PrepareHandler>();
        return this;
    }

    /// <summary>
    /// Register a prepare handler for an event type_url suffix.
    /// Must be called after Domain() to set context.
    /// </summary>
    public EventRouter Prepare(string suffix, PrepareHandler handler)
    {
        if (_currentDomain == null)
            throw new InvalidOperationException("Must call Domain() before Prepare()");
        _prepareHandlers[_currentDomain][suffix] = handler;
        return this;
    }

    /// <summary>
    /// Register a handler for an event type_url suffix in current domain.
    /// Must be called after Domain() to set context.
    /// </summary>
    public EventRouter On(string suffix, EventHandler handler)
    {
        if (_currentDomain == null)
            throw new InvalidOperationException("Must call Domain() before On()");
        _handlers[_currentDomain].Add((suffix, handler));
        return this;
    }

    /// <summary>
    /// Auto-derive subscriptions from registered handlers.
    /// </summary>
    /// <returns>Dictionary of domain to event types.</returns>
    public Dictionary<string, List<string>> Subscriptions()
    {
        var result = new Dictionary<string, List<string>>();
        foreach (var (domain, handlers) in _handlers)
        {
            if (handlers.Count > 0)
                result[domain] = handlers.Select(h => h.Suffix).ToList();
        }
        return result;
    }

    /// <summary>
    /// Get destinations needed for the given source events.
    /// Routes based on source domain.
    /// </summary>
    public List<Angzarr.Cover> PrepareDestinations(Angzarr.EventBook book)
    {
        var sourceDomain = book.Cover?.Domain ?? "";
        if (!_prepareHandlers.TryGetValue(sourceDomain, out var domainHandlers))
            return new List<Angzarr.Cover>();

        var root = book.Cover?.Root;
        var destinations = new List<Angzarr.Cover>();

        foreach (var page in book.Pages)
        {
            if (page.Event == null) continue;
            foreach (var (suffix, handler) in domainHandlers)
            {
                if (page.Event.TypeUrl.EndsWith(suffix))
                {
                    destinations.AddRange(handler(page.Event, root));
                    break;
                }
            }
        }
        return destinations;
    }

    /// <summary>
    /// Dispatch all events in an EventBook to registered handlers.
    /// Routes based on source domain and event type suffix.
    /// </summary>
    public List<Angzarr.CommandBook> Dispatch(
        Angzarr.EventBook book,
        List<Angzarr.EventBook>? destinations = null)
    {
        var sourceDomain = book.Cover?.Domain ?? "";
        if (!_handlers.TryGetValue(sourceDomain, out var domainHandlers))
            return new List<Angzarr.CommandBook>();

        var root = book.Cover?.Root?.Value.ToByteArray();
        var correlationId = book.Cover?.CorrelationId ?? "";
        var commands = new List<Angzarr.CommandBook>();

        foreach (var page in book.Pages)
        {
            if (page.Event == null) continue;
            foreach (var (suffix, handler) in domainHandlers)
            {
                if (page.Event.TypeUrl.EndsWith(suffix))
                {
                    commands.AddRange(handler(page.Event, root, correlationId, destinations));
                    break;
                }
            }
        }
        return commands;
    }

    /// <summary>
    /// Return the first registered domain (for backwards compatibility).
    /// </summary>
    [Obsolete("Use Subscriptions() instead")]
    public string InputDomain() => _handlers.Keys.FirstOrDefault() ?? "";

}

/// <summary>
/// Delegate for state appliers.
/// </summary>
public delegate void StateApplier<TState>(TState state, IMessage eventMessage);

/// <summary>
/// Fluent state reconstruction from events.
/// </summary>
public class StateRouter<TState> where TState : new()
{
    private readonly Dictionary<string, (Type EventType, Action<TState, Any> Applier)> _appliers = new();

    /// <summary>
    /// Register an event applier.
    /// </summary>
    public StateRouter<TState> On<TEvent>(Action<TState, TEvent> applier) where TEvent : IMessage, new()
    {
        var suffix = typeof(TEvent).Name;
        _appliers[suffix] = (typeof(TEvent), (state, any) =>
        {
            var evt = any.Unpack<TEvent>();
            applier(state, evt);
        });
        return this;
    }

    /// <summary>
    /// Rebuild state from an EventBook.
    /// </summary>
    public TState WithEventBook(Angzarr.EventBook? book)
    {
        var state = new TState();
        if (book == null) return state;

        foreach (var page in book.Pages)
        {
            if (page.Event == null) continue;
            ApplyEvent(state, page.Event);
        }
        return state;
    }

    private void ApplyEvent(TState state, Any eventAny)
    {
        foreach (var (suffix, (_, applier)) in _appliers)
        {
            if (eventAny.TypeUrl.EndsWith(suffix))
            {
                applier(state, eventAny);
                return;
            }
        }
        // Unknown event type - silently ignore (forward compatibility)
    }
}

/// <summary>
/// Delegate for upcaster handlers.
/// </summary>
public delegate Any UpcasterHandler(Any eventAny);

/// <summary>
/// Event version transformer.
/// </summary>
public class UpcasterRouter
{
    private readonly string _domain;
    private readonly List<(string Suffix, UpcasterHandler Handler, Type ToType)> _handlers = new();

    public UpcasterRouter(string domain)
    {
        _domain = domain;
    }

    /// <summary>
    /// Register an upcaster handler.
    /// </summary>
    public UpcasterRouter On(string suffix, UpcasterHandler handler, Type? toType = null)
    {
        _handlers.Add((suffix, handler, toType ?? typeof(object)));
        return this;
    }

    /// <summary>
    /// Transform a list of events to current versions.
    /// </summary>
    public List<Angzarr.EventPage> Upcast(IEnumerable<Angzarr.EventPage> events)
    {
        var result = new List<Angzarr.EventPage>();

        foreach (var page in events)
        {
            if (page.Event == null)
            {
                result.Add(page);
                continue;
            }

            var transformed = false;
            foreach (var (suffix, handler, _) in _handlers)
            {
                if (page.Event.TypeUrl.EndsWith(suffix))
                {
                    var newEvent = handler(page.Event);
                    var newPage = new Angzarr.EventPage
                    {
                        Event = newEvent,
                        Sequence = page.Sequence,
                        CreatedAt = page.CreatedAt
                    };
                    result.Add(newPage);
                    transformed = true;
                    break;
                }
            }

            if (!transformed)
                result.Add(page);
        }

        return result;
    }
}
