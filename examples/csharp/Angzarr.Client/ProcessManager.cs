using System.Reflection;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Base class for stateful process managers using the OO pattern.
///
/// Subclasses must:
/// - Override Name property
/// - Override CreateEmptyState() and ApplyEvent()
/// - Decorate event handlers with [ReactsTo(typeof(EventType), InputDomain = "...")]
/// - Optionally decorate prepare handlers with [Prepares(typeof(EventType))]
/// - Optionally decorate rejection handlers with [Rejected("domain", "command")]
/// </summary>
public abstract class ProcessManager<TState> where TState : class
{
    private Angzarr.EventBook _eventBook;
    private TState? _state;
    private readonly List<Any> _newEvents = new();

    // Dispatch tables built via reflection on first use
    private static readonly Dictionary<Type, Dictionary<string, (MethodInfo Method, Type EventType, string? InputDomain, string? OutputDomain)>> _dispatchTables = new();
    private static readonly Dictionary<Type, Dictionary<string, (MethodInfo Method, Type EventType)>> _prepareTables = new();
    private static readonly Dictionary<Type, Dictionary<string, MethodInfo>> _rejectionTables = new();
    private static readonly Dictionary<Type, Dictionary<string, List<string>>> _inputDomains = new();

    /// <summary>
    /// The name of this process manager.
    /// </summary>
    public abstract string Name { get; }

    /// <summary>
    /// Create an empty state instance.
    /// </summary>
    protected abstract TState CreateEmptyState();

    /// <summary>
    /// Apply a single event to state.
    /// </summary>
    protected abstract void ApplyEvent(TState state, Any eventAny);

    protected ProcessManager(Angzarr.EventBook? processState = null)
    {
        _eventBook = processState ?? new Angzarr.EventBook();
        EnsureDispatchTablesBuilt();
    }

    private void EnsureDispatchTablesBuilt()
    {
        var type = GetType();
        if (_dispatchTables.ContainsKey(type)) return;

        lock (_dispatchTables)
        {
            if (_dispatchTables.ContainsKey(type)) return;

            var dispatch = new Dictionary<string, (MethodInfo, Type, string?, string?)>();
            var domains = new Dictionary<string, List<string>>();

            foreach (var method in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
            {
                var attr = method.GetCustomAttribute<ReactsToAttribute>();
                if (attr != null)
                {
                    var suffix = attr.EventType.Name;
                    dispatch[suffix] = (method, attr.EventType, attr.InputDomain, attr.OutputDomain);

                    // Track input domains
                    if (!string.IsNullOrEmpty(attr.InputDomain))
                    {
                        if (!domains.TryGetValue(attr.InputDomain, out var types))
                        {
                            types = new List<string>();
                            domains[attr.InputDomain] = types;
                        }
                        types.Add(suffix);
                    }
                }
            }
            _dispatchTables[type] = dispatch;
            _inputDomains[type] = domains;

            // Build prepare handler dispatch table
            var prepares = new Dictionary<string, (MethodInfo, Type)>();
            foreach (var method in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
            {
                var attr = method.GetCustomAttribute<PreparesAttribute>();
                if (attr != null)
                {
                    var suffix = attr.EventType.Name;
                    prepares[suffix] = (method, attr.EventType);
                }
            }
            _prepareTables[type] = prepares;

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
    /// Phase 1: Declare additional destinations needed.
    /// </summary>
    public static List<Angzarr.Cover> PrepareDestinations<T>(
        Angzarr.EventBook trigger,
        Angzarr.EventBook processState) where T : ProcessManager<TState>, new()
    {
        var pm = (T)Activator.CreateInstance(typeof(T), processState)!;
        var destinations = new List<Angzarr.Cover>();

        foreach (var page in trigger.Pages)
        {
            if (page.Event != null)
                destinations.AddRange(pm.Prepare(page.Event));
        }

        return destinations;
    }

    /// <summary>
    /// Phase 2: Handle a trigger event with current process state.
    /// </summary>
    public static (List<Angzarr.CommandBook> Commands, Angzarr.EventBook ProcessEvents) Handle<T>(
        Angzarr.EventBook trigger,
        Angzarr.EventBook processState,
        List<Angzarr.EventBook>? destinations = null) where T : ProcessManager<TState>, new()
    {
        var pm = (T)Activator.CreateInstance(typeof(T), processState)!;
        var root = trigger.Cover?.Root?.Value.ToByteArray();
        var correlationId = trigger.Cover?.CorrelationId ?? "";

        var commands = new List<Angzarr.CommandBook>();
        foreach (var page in trigger.Pages)
        {
            if (page.Event != null)
                commands.AddRange(pm.Dispatch(page.Event, root, correlationId, destinations));
        }

        return (commands, pm.ProcessEvents());
    }

    /// <summary>
    /// Prepare destinations for a single event.
    /// </summary>
    public List<Angzarr.Cover> Prepare(Any eventAny)
    {
        var prepareTable = _prepareTables[GetType()];
        foreach (var (suffix, (method, eventType)) in prepareTable)
        {
            if (eventAny.TypeUrl.EndsWith(suffix))
            {
                var unpackMethod = typeof(Any).GetMethod("Unpack")!.MakeGenericMethod(eventType);
                var evt = unpackMethod.Invoke(eventAny, null);
                var result = method.Invoke(this, new[] { evt });
                return result as List<Angzarr.Cover> ?? new List<Angzarr.Cover>();
            }
        }
        return new List<Angzarr.Cover>();
    }

    /// <summary>
    /// Dispatch a single event to the matching handler.
    /// </summary>
    public List<Angzarr.CommandBook> Dispatch(
        Any eventAny,
        byte[]? root = null,
        string correlationId = "",
        List<Angzarr.EventBook>? destinations = null)
    {
        var dispatchTable = _dispatchTables[GetType()];
        foreach (var (suffix, (method, eventType, _, outputDomain)) in dispatchTable)
        {
            if (eventAny.TypeUrl.EndsWith(suffix))
            {
                var unpackMethod = typeof(Any).GetMethod("Unpack")!.MakeGenericMethod(eventType);
                var evt = unpackMethod.Invoke(eventAny, null);

                // Check if method accepts destinations parameter
                var parameters = method.GetParameters();
                object? result;
                if (parameters.Any(p => p.Name == "destinations"))
                {
                    result = method.Invoke(this, new object?[] { evt, destinations ?? new List<Angzarr.EventBook>() });
                }
                else
                {
                    result = method.Invoke(this, new[] { evt });
                }

                return PackCommands(result, outputDomain, root, correlationId);
            }
        }
        return new List<Angzarr.CommandBook>();
    }

    /// <summary>
    /// Handle rejection notification.
    /// </summary>
    public (Angzarr.EventBook? Events, Angzarr.RevocationResponse Revocation) HandleRevocation(
        Angzarr.Notification notification)
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
                return (ProcessEvents(), new Angzarr.RevocationResponse
                {
                    EmitSystemRevocation = false,
                    Reason = $"ProcessManager {Name} handled rejection for {key}"
                });
            }
        }

        return (null, new Angzarr.RevocationResponse
        {
            EmitSystemRevocation = true,
            Reason = $"ProcessManager {Name} has no custom compensation for {domain}/{commandSuffix}"
        });
    }

    /// <summary>
    /// Get the current state.
    /// </summary>
    public TState State => GetState();

    /// <summary>
    /// Return new process events for persistence.
    /// </summary>
    public Angzarr.EventBook ProcessEvents()
    {
        var book = new Angzarr.EventBook();
        book.Pages.AddRange(_newEvents.Select(e => new Angzarr.EventPage { Event = e }));
        return book;
    }

    /// <summary>
    /// Build a component descriptor for topology discovery.
    /// </summary>
    public Descriptor Descriptor()
    {
        var domains = _inputDomains[GetType()];
        var inputs = domains.Select(kv => new TargetDesc(kv.Key, kv.Value)).ToList();
        return new Descriptor(Name, ComponentTypes.ProcessManager, inputs);
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
        return state;
    }

    /// <summary>
    /// Pack event, apply to cached state, record for persistence.
    /// </summary>
    protected void ApplyAndRecord(IMessage eventMessage)
    {
        var eventAny = Any.Pack(eventMessage, "type.googleapis.com/");

        if (_state != null)
            ApplyEvent(_state, eventAny);

        _newEvents.Add(eventAny);
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

    private List<Angzarr.CommandBook> PackCommands(
        object? result,
        string? outputDomain,
        byte[]? root,
        string correlationId)
    {
        if (result == null) return new List<Angzarr.CommandBook>();

        // Handle pre-packed CommandBooks
        if (result is Angzarr.CommandBook book)
            return new List<Angzarr.CommandBook> { book };
        if (result is List<Angzarr.CommandBook> books)
            return books;

        var commands = new List<IMessage>();
        if (result is System.Collections.IEnumerable enumerable && result is not IMessage)
        {
            foreach (var item in enumerable)
            {
                if (item is IMessage msg)
                    commands.Add(msg);
            }
        }
        else if (result is IMessage message)
        {
            commands.Add(message);
        }

        return commands.Select(cmd => PackCommand(cmd, outputDomain ?? "", root, correlationId)).ToList();
    }

    private Angzarr.CommandBook PackCommand(IMessage cmd, string domain, byte[]? root, string correlationId)
    {
        var cmdAny = Any.Pack(cmd, "type.googleapis.com/");
        var cover = new Angzarr.Cover
        {
            Domain = domain,
            CorrelationId = correlationId
        };
        if (root != null)
            cover.Root = new Angzarr.UUID { Value = Google.Protobuf.ByteString.CopyFrom(root) };

        return new Angzarr.CommandBook
        {
            Cover = cover,
            Pages = { new Angzarr.CommandPage { Command = cmdAny } }
        };
    }
}

/// <summary>
/// Helper utilities for ProcessManager.
/// </summary>
internal static class Helpers
{
    /// <summary>
    /// Extract the type name suffix from a type_url.
    /// </summary>
    public static string TypeNameFromUrl(string typeUrl)
    {
        var lastSlash = typeUrl.LastIndexOf('/');
        return lastSlash >= 0 ? typeUrl.Substring(lastSlash + 1) : typeUrl;
    }
}
