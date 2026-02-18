using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Component type constants for descriptors.
/// </summary>
public static class ComponentTypes
{
    public const string Aggregate = "aggregate";
    public const string Saga = "saga";
    public const string ProcessManager = "process_manager";
    public const string Projector = "projector";
    public const string Upcaster = "upcaster";
}

/// <summary>
/// Error message constants.
/// </summary>
public static class ErrorMessages
{
    public const string UnknownCommand = "Unknown command type";
    public const string NoCommandPages = "No command pages";
}

/// <summary>
/// Describes what a component subscribes to or sends to.
/// </summary>
public record TargetDesc(string Domain, List<string> Types);

/// <summary>
/// Describes a component for topology discovery.
/// </summary>
public record Descriptor(string Name, string ComponentType, List<TargetDesc> Inputs);

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
/// Delegate for rejection handlers.
/// </summary>
public delegate Angzarr.EventBook RejectionHandler<TState>(
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
                var events = handler(notification, state);
                return new Angzarr.BusinessResponse { Events = events };
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

    /// <summary>
    /// Build a component descriptor from registered handlers.
    /// </summary>
    public Descriptor Descriptor()
    {
        return new Descriptor(
            _domain,
            ComponentTypes.Aggregate,
            new List<TargetDesc> { new(_domain, Types()) });
    }

    /// <summary>
    /// Return registered command type suffixes.
    /// </summary>
    public List<string> Types() => _handlers.Select(h => h.Suffix).ToList();
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
/// DRY event dispatcher for sagas.
/// </summary>
public class EventRouter
{
    private readonly string _name;
    private readonly string _inputDomain;
    private readonly Dictionary<string, List<string>> _outputTargets = new();
    private readonly List<(string Suffix, EventHandler Handler)> _handlers = new();
    private readonly Dictionary<string, PrepareHandler> _prepareHandlers = new();

    public EventRouter(string name, string inputDomain)
    {
        _name = name;
        _inputDomain = inputDomain;
    }

    /// <summary>
    /// Declare an output domain and command type.
    /// </summary>
    public EventRouter Sends(string domain, string commandType)
    {
        if (!_outputTargets.TryGetValue(domain, out var types))
        {
            types = new List<string>();
            _outputTargets[domain] = types;
        }
        types.Add(commandType);
        return this;
    }

    /// <summary>
    /// Register a prepare handler for an event type_url suffix.
    /// </summary>
    public EventRouter Prepare(string suffix, PrepareHandler handler)
    {
        _prepareHandlers[suffix] = handler;
        return this;
    }

    /// <summary>
    /// Register a handler for an event type_url suffix.
    /// </summary>
    public EventRouter On(string suffix, EventHandler handler)
    {
        _handlers.Add((suffix, handler));
        return this;
    }

    /// <summary>
    /// Get destinations needed for the given source events.
    /// </summary>
    public List<Angzarr.Cover> PrepareDestinations(Angzarr.EventBook book)
    {
        var root = book.Cover?.Root;
        var destinations = new List<Angzarr.Cover>();

        foreach (var page in book.Pages)
        {
            if (page.Event == null) continue;
            foreach (var (suffix, handler) in _prepareHandlers)
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
    /// </summary>
    public List<Angzarr.CommandBook> Dispatch(
        Angzarr.EventBook book,
        List<Angzarr.EventBook>? destinations = null)
    {
        var root = book.Cover?.Root?.Value.ToByteArray();
        var correlationId = book.Cover?.CorrelationId ?? "";
        var commands = new List<Angzarr.CommandBook>();

        foreach (var page in book.Pages)
        {
            if (page.Event == null) continue;
            foreach (var (suffix, handler) in _handlers)
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
    /// Build a component descriptor from registered handlers.
    /// </summary>
    public Descriptor Descriptor()
    {
        return new Descriptor(
            _name,
            ComponentTypes.Saga,
            new List<TargetDesc> { new(_inputDomain, Types()) });
    }

    /// <summary>
    /// Return registered event type suffixes.
    /// </summary>
    public List<string> Types() => _handlers.Select(h => h.Suffix).ToList();

    /// <summary>
    /// Return output domain names.
    /// </summary>
    public List<string> OutputDomains() => _outputTargets.Keys.ToList();

    /// <summary>
    /// Return command types for a given output domain.
    /// </summary>
    public List<string> OutputTypes(string domain) =>
        _outputTargets.TryGetValue(domain, out var types) ? types : new List<string>();
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
                        Num = page.Num,
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

    /// <summary>
    /// Build a component descriptor.
    /// </summary>
    public Descriptor Descriptor()
    {
        return new Descriptor(
            $"upcaster-{_domain}",
            ComponentTypes.Upcaster,
            new List<TargetDesc> { new(_domain, Types()) });
    }

    /// <summary>
    /// Return registered old event type suffixes.
    /// </summary>
    public List<string> Types() => _handlers.Select(h => h.Suffix).ToList();
}
