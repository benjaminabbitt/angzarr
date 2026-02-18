using System.Reflection;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Base class for stateless event-to-command sagas using the OO pattern.
///
/// Subclasses must:
/// - Override Name, InputDomain, OutputDomain properties
/// - Decorate event handlers with [ReactsTo(typeof(EventType))]
/// - Optionally decorate prepare handlers with [Prepares(typeof(EventType))]
/// </summary>
public abstract class Saga
{
    // Dispatch tables built via reflection on first use
    private static readonly Dictionary<Type, Dictionary<string, (MethodInfo Method, Type EventType)>> _dispatchTables = new();
    private static readonly Dictionary<Type, Dictionary<string, (MethodInfo Method, Type EventType)>> _prepareTables = new();

    /// <summary>
    /// The name of this saga (e.g., "saga-order-fulfillment").
    /// </summary>
    public abstract string Name { get; }

    /// <summary>
    /// The domain this saga listens to.
    /// </summary>
    public abstract string InputDomain { get; }

    /// <summary>
    /// The domain this saga sends commands to.
    /// </summary>
    public abstract string OutputDomain { get; }

    protected Saga()
    {
        EnsureDispatchTablesBuilt();
    }

    private void EnsureDispatchTablesBuilt()
    {
        var type = GetType();
        if (_dispatchTables.ContainsKey(type)) return;

        lock (_dispatchTables)
        {
            if (_dispatchTables.ContainsKey(type)) return;

            // Build event handler dispatch table
            var dispatch = new Dictionary<string, (MethodInfo, Type)>();
            foreach (var method in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
            {
                var attr = method.GetCustomAttribute<ReactsToAttribute>();
                if (attr != null)
                {
                    var suffix = attr.EventType.Name;
                    dispatch[suffix] = (method, attr.EventType);
                }
            }
            _dispatchTables[type] = dispatch;

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
        }
    }

    /// <summary>
    /// Phase 1: Declare destination aggregates needed.
    /// </summary>
    public static List<Angzarr.Cover> PrepareDestinations<T>(Angzarr.EventBook source) where T : Saga, new()
    {
        var saga = new T();
        var destinations = new List<Angzarr.Cover>();

        foreach (var page in source.Pages)
        {
            if (page.Event != null)
                destinations.AddRange(saga.Prepare(page.Event));
        }

        return destinations;
    }

    /// <summary>
    /// Phase 2: Process EventBook and return commands.
    /// </summary>
    public static List<Angzarr.CommandBook> Execute<T>(
        Angzarr.EventBook source,
        List<Angzarr.EventBook>? destinations = null) where T : Saga, new()
    {
        var saga = new T();
        var root = source.Cover?.Root?.Value.ToByteArray();
        var correlationId = source.Cover?.CorrelationId ?? "";

        var commands = new List<Angzarr.CommandBook>();
        foreach (var page in source.Pages)
        {
            if (page.Event != null)
                commands.AddRange(saga.Dispatch(page.Event, root, correlationId, destinations));
        }

        return commands;
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
        foreach (var (suffix, (method, eventType)) in dispatchTable)
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

                return PackCommands(result, root, correlationId);
            }
        }
        return new List<Angzarr.CommandBook>();
    }

    private List<Angzarr.CommandBook> PackCommands(object? result, byte[]? root, string correlationId)
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

        return commands.Select(cmd => PackCommand(cmd, root, correlationId)).ToList();
    }

    private Angzarr.CommandBook PackCommand(IMessage cmd, byte[]? root, string correlationId)
    {
        var cmdAny = Any.Pack(cmd, "type.googleapis.com/");
        var cover = new Angzarr.Cover
        {
            Domain = OutputDomain,
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

    /// <summary>
    /// Build a component descriptor for topology discovery.
    /// </summary>
    public Descriptor Descriptor()
    {
        var dispatchTable = _dispatchTables[GetType()];
        return new Descriptor(
            Name,
            ComponentTypes.Saga,
            new List<TargetDesc> { new(InputDomain, dispatchTable.Keys.ToList()) });
    }
}
