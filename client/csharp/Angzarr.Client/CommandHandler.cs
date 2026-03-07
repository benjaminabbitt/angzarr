using System.Reflection;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Base class for event-sourced command handlers using the OO pattern.
///
/// Subclasses must:
/// - Override Domain property
/// - Override CreateEmptyState()
/// - Decorate command handlers with [Handles(typeof(CommandType))]
/// - Decorate event appliers with [Applies(typeof(EventType))]
/// - Optionally decorate rejection handlers with [Rejected("domain", "command")]
/// </summary>
public abstract class CommandHandler<TState>
    where TState : class
{
    private Angzarr.EventBook _eventBook;
    private TState? _state;

    // Dispatch tables built via reflection on first use
    private static readonly Dictionary<
        Type,
        Dictionary<string, (MethodInfo Method, Type CmdType)>
    > _dispatchTables = new();
    private static readonly Dictionary<
        Type,
        Dictionary<string, (MethodInfo Method, Type EventType)>
    > _applierTables = new();
    private static readonly Dictionary<Type, Dictionary<string, MethodInfo>> _rejectionTables =
        new();

    /// <summary>
    /// The domain this command handler belongs to.
    /// </summary>
    public abstract string Domain { get; }

    /// <summary>
    /// Create an empty state instance.
    /// </summary>
    protected abstract TState CreateEmptyState();

    protected CommandHandler(Angzarr.EventBook? eventBook = null)
    {
        _eventBook = eventBook ?? new Angzarr.EventBook();
        EnsureDispatchTablesBuilt();
    }

    private void EnsureDispatchTablesBuilt()
    {
        var type = GetType();
        if (_dispatchTables.ContainsKey(type))
            return;

        lock (_dispatchTables)
        {
            if (_dispatchTables.ContainsKey(type))
                return;

            // Build command handler dispatch table
            var dispatch = new Dictionary<string, (MethodInfo, Type)>();
            foreach (
                var method in type.GetMethods(
                    BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic
                )
            )
            {
                var attr = method.GetCustomAttribute<HandlesAttribute>();
                if (attr != null)
                {
                    var suffix = attr.EventType.Name;
                    dispatch[suffix] = (method, attr.EventType);
                }
            }
            _dispatchTables[type] = dispatch;

            // Build event applier dispatch table
            var appliers = new Dictionary<string, (MethodInfo, Type)>();
            foreach (
                var method in type.GetMethods(
                    BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic
                )
            )
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
            foreach (
                var method in type.GetMethods(
                    BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic
                )
            )
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
    public static Angzarr.BusinessResponse Handle<T>(Angzarr.ContextualCommand request)
        where T : CommandHandler<TState>, new()
    {
        var priorEvents = request.Events;
        var handler = (T)Activator.CreateInstance(typeof(T), priorEvents)!;

        if (request.Command.Pages.Count == 0)
            throw new InvalidArgumentError("No command pages");

        var commandAny = request.Command.Pages[0].Command;

        // Check for Notification (rejection/compensation)
        if (commandAny.TypeUrl.EndsWith("Notification"))
        {
            var notification = commandAny.Unpack<Angzarr.Notification>();
            return handler.HandleRevocation(notification);
        }

        handler.Dispatch(commandAny);
        return new Angzarr.BusinessResponse { Events = handler.EventBook() };
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
                Reason =
                    $"CommandHandler {Domain} has no custom compensation for {domain}/{commandSuffix}",
            },
        };
    }

    private void HandleResult(object? result)
    {
        if (result == null)
            return;

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
    /// Check if this command handler has prior events.
    /// </summary>
    public bool Exists => _state != null || _eventBook.Pages.Count > 0;

    /// <summary>
    /// Get the event book for persistence.
    /// </summary>
    public Angzarr.EventBook EventBook() => _eventBook;

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

    /// <summary>
    /// Apply a single event to state.
    /// Default implementation uses [Applies] attributes to dispatch.
    /// Override this method if you need custom dispatch logic.
    /// </summary>
    protected virtual void ApplyEvent(TState state, Any eventAny)
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

    /// <summary>
    /// Rehydrate state from an event book.
    /// Alternative to constructor injection for testing and simple use cases.
    /// </summary>
    public void Rehydrate(Angzarr.EventBook newEventBook)
    {
        _eventBook = newEventBook ?? new Angzarr.EventBook();
        _state = default; // Force rebuild on next State access
    }

    /// <summary>
    /// Handle a command and return the resulting event.
    /// Convenience method for testing and simple use cases.
    /// </summary>
    public IMessage HandleCommand(IMessage command)
    {
        var commandAny = Any.Pack(command, "type.googleapis.com/");
        var typeUrl = commandAny.TypeUrl;
        var dispatchTable = _dispatchTables[GetType()];

        foreach (var (suffix, (method, cmdType)) in dispatchTable)
        {
            if (typeUrl.EndsWith(suffix))
            {
                _ = State; // Ensure state is built
                var result = method.Invoke(this, new[] { command });
                if (result is IMessage msg)
                {
                    ApplyAndRecord(msg);
                    return msg;
                }
                throw new InvalidOperationException($"Handler for {suffix} returned null");
            }
        }

        throw new InvalidArgumentError($"Unknown command: {typeUrl}");
    }
}
