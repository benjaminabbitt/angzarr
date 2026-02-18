using System.Reflection;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Base class for event-sourced aggregates using the OO pattern.
///
/// Subclasses must:
/// - Override Domain property
/// - Override CreateEmptyState()
/// - Decorate command handlers with [Handles(typeof(CommandType))]
/// - Decorate event appliers with [Applies(typeof(EventType))]
/// - Optionally decorate rejection handlers with [Rejected("domain", "command")]
/// </summary>
public abstract class Aggregate<TState> where TState : class
{
    private Angzarr.EventBook _eventBook;
    private TState? _state;

    // Dispatch tables built via reflection on first use
    private static readonly Dictionary<Type, Dictionary<string, (MethodInfo Method, Type CmdType)>> _dispatchTables = new();
    private static readonly Dictionary<Type, Dictionary<string, (MethodInfo Method, Type EventType)>> _applierTables = new();
    private static readonly Dictionary<Type, Dictionary<string, MethodInfo>> _rejectionTables = new();

    /// <summary>
    /// The domain this aggregate belongs to.
    /// </summary>
    public abstract string Domain { get; }

    /// <summary>
    /// Create an empty state instance.
    /// </summary>
    protected abstract TState CreateEmptyState();

    protected Aggregate(Angzarr.EventBook? eventBook = null)
    {
        _eventBook = eventBook ?? new Angzarr.EventBook();
        EnsureDispatchTablesBuilt();
    }

    private void EnsureDispatchTablesBuilt()
    {
        var type = GetType();
        if (_dispatchTables.ContainsKey(type)) return;

        lock (_dispatchTables)
        {
            if (_dispatchTables.ContainsKey(type)) return;

            // Build command handler dispatch table
            var dispatch = new Dictionary<string, (MethodInfo, Type)>();
            foreach (var method in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
            {
                var attr = method.GetCustomAttribute<HandlesAttribute>();
                if (attr != null)
                {
                    var suffix = attr.CommandType.Name;
                    dispatch[suffix] = (method, attr.CommandType);
                }
            }
            _dispatchTables[type] = dispatch;

            // Build event applier dispatch table
            var appliers = new Dictionary<string, (MethodInfo, Type)>();
            foreach (var method in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
            {
                var attr = method.GetCustomAttribute<AppliesAttribute>();
                if (attr != null)
                {
                    var suffix = attr.EventType.Name;
                    appliers[suffix] = (method, attr.EventType);
                }
            }
            _applierTables[type] = appliers;

            // Build rejection handler dispatch table
            var rejections = new Dictionary<string, MethodInfo>();
            foreach (var method in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
            {
                var attr = method.GetCustomAttribute<RejectedAttribute>();
                if (attr != null)
                {
                    var key = $"{attr.Domain}/{attr.Command}";
                    rejections[key] = method;
                }
            }
            _rejectionTables[type] = rejections;
        }
    }

    /// <summary>
    /// Handle a gRPC request.
    /// </summary>
    public static Angzarr.BusinessResponse Handle<T>(Angzarr.ContextualCommand request) where T : Aggregate<TState>, new()
    {
        var priorEvents = request.Events;
        var agg = (T)Activator.CreateInstance(typeof(T), priorEvents)!;

        if (request.Command.Pages.Count == 0)
            throw new InvalidArgumentError("No command pages");

        var commandAny = request.Command.Pages[0].Command;

        // Check for Notification (rejection/compensation)
        if (commandAny.TypeUrl.EndsWith("Notification"))
        {
            var notification = commandAny.Unpack<Angzarr.Notification>();
            return agg.HandleRevocation(notification);
        }

        agg.Dispatch(commandAny);
        return new Angzarr.BusinessResponse { Events = agg.EventBook() };
    }

    /// <summary>
    /// Dispatch a command to the matching [Handles] method.
    /// </summary>
    public void Dispatch(Any commandAny)
    {
        var typeUrl = commandAny.TypeUrl;
        var dispatchTable = _dispatchTables[GetType()];

        foreach (var (suffix, (method, cmdType)) in dispatchTable)
        {
            if (typeUrl.EndsWith(suffix))
            {
                var unpackMethod = typeof(Any).GetMethod("Unpack")!.MakeGenericMethod(cmdType);
                var cmd = unpackMethod.Invoke(commandAny, null);
                var result = method.Invoke(this, new[] { cmd });
                HandleResult(result);
                return;
            }
        }

        throw new InvalidArgumentError($"Unknown command: {typeUrl}");
    }

    /// <summary>
    /// Handle rejection notification.
    /// </summary>
    public Angzarr.BusinessResponse HandleRevocation(Angzarr.Notification notification)
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

        var rejectionTable = _rejectionTables[GetType()];
        foreach (var (key, method) in rejectionTable)
        {
            var parts = key.Split('/');
            if (parts[0] == domain && commandSuffix.EndsWith(parts[1]))
            {
                _ = State; // Ensure state is built
                var result = method.Invoke(this, new object[] { notification });
                HandleResult(result);
                return new Angzarr.BusinessResponse { Events = EventBook() };
            }
        }

        return new Angzarr.BusinessResponse
        {
            Revocation = new Angzarr.RevocationResponse
            {
                EmitSystemRevocation = true,
                Reason = $"Aggregate {Domain} has no custom compensation for {domain}/{commandSuffix}"
            }
        };
    }

    private void HandleResult(object? result)
    {
        if (result == null) return;

        if (result is System.Collections.IEnumerable enumerable && result is not IMessage)
        {
            foreach (var item in enumerable)
            {
                if (item is IMessage msg)
                    ApplyAndRecord(msg);
            }
        }
        else if (result is IMessage message)
        {
            ApplyAndRecord(message);
        }
    }

    /// <summary>
    /// Get the current state.
    /// </summary>
    public TState State => GetState();

    /// <summary>
    /// Check if this aggregate has prior events.
    /// </summary>
    public bool Exists => _state != null || _eventBook.Pages.Count > 0;

    /// <summary>
    /// Get the event book for persistence.
    /// </summary>
    public Angzarr.EventBook EventBook() => _eventBook;

    /// <summary>
    /// Build a component descriptor for topology discovery.
    /// </summary>
    public Descriptor Descriptor()
    {
        var dispatchTable = _dispatchTables[GetType()];
        return new Descriptor(
            Domain,
            ComponentTypes.Aggregate,
            new List<TargetDesc> { new(Domain, dispatchTable.Keys.ToList()) });
    }

    private TState GetState()
    {
        if (_state == null)
            _state = Rebuild();
        return _state;
    }

    private TState Rebuild()
    {
        var state = CreateEmptyState();
        foreach (var page in _eventBook.Pages)
        {
            if (page.Event != null)
                ApplyEvent(state, page.Event);
        }
        // Clear consumed events - only new events will be in the book
        _eventBook.Pages.Clear();
        return state;
    }

    /// <summary>
    /// Pack event, apply to cached state, add to event book.
    /// Called by [Handles] decorated methods.
    /// </summary>
    protected void ApplyAndRecord(IMessage eventMessage)
    {
        var eventAny = Any.Pack(eventMessage, "type.googleapis.com/");

        // Apply directly to cached state
        if (_state != null)
            ApplyEvent(_state, eventAny);

        // Record in event book
        var page = new Angzarr.EventPage { Event = eventAny };
        _eventBook.Pages.Add(page);
    }

    private void ApplyEvent(TState state, Any eventAny)
    {
        var applierTable = _applierTables[GetType()];
        foreach (var (suffix, (method, eventType)) in applierTable)
        {
            if (eventAny.TypeUrl.EndsWith(suffix))
            {
                var unpackMethod = typeof(Any).GetMethod("Unpack")!.MakeGenericMethod(eventType);
                var evt = unpackMethod.Invoke(eventAny, null);
                method.Invoke(this, new[] { state, evt });
                return;
            }
        }
        // Unknown event type - silently ignore (forward compatibility)
    }
}
