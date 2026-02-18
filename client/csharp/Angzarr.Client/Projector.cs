using System.Reflection;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Base class for event-driven projectors using the OO pattern.
///
/// Subclasses must:
/// - Override Name and InputDomain properties
/// - Decorate event handlers with [Projects(typeof(EventType))]
/// </summary>
public abstract class Projector
{
    // Dispatch table built via reflection on first use
    private static readonly Dictionary<Type, Dictionary<string, (MethodInfo Method, Type EventType)>> _dispatchTables = new();

    /// <summary>
    /// The name of this projector (e.g., "projector-inventory-stock").
    /// </summary>
    public abstract string Name { get; }

    /// <summary>
    /// The domain this projector listens to.
    /// </summary>
    public abstract string InputDomain { get; }

    protected Projector()
    {
        EnsureDispatchTableBuilt();
    }

    private void EnsureDispatchTableBuilt()
    {
        var type = GetType();
        if (_dispatchTables.ContainsKey(type)) return;

        lock (_dispatchTables)
        {
            if (_dispatchTables.ContainsKey(type)) return;

            var dispatch = new Dictionary<string, (MethodInfo, Type)>();
            foreach (var method in type.GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic))
            {
                var attr = method.GetCustomAttribute<ProjectsAttribute>();
                if (attr != null)
                {
                    var suffix = attr.EventType.Name;
                    dispatch[suffix] = (method, attr.EventType);
                }
            }
            _dispatchTables[type] = dispatch;
        }
    }

    /// <summary>
    /// Process an EventBook and return projection from last handled event.
    /// </summary>
    public static Angzarr.Projection Handle<T>(Angzarr.EventBook source) where T : Projector, new()
    {
        var projector = new T();
        var lastProjection = new Angzarr.Projection();

        foreach (var page in source.Pages)
        {
            if (page.Event != null)
            {
                var result = projector.Dispatch(page.Event);
                if (!string.IsNullOrEmpty(result.Projector))
                    lastProjection = result;
            }
        }

        return lastProjection;
    }

    /// <summary>
    /// Dispatch a single event to the matching handler.
    /// </summary>
    public Angzarr.Projection Dispatch(Any eventAny)
    {
        var dispatchTable = _dispatchTables[GetType()];
        foreach (var (suffix, (method, eventType)) in dispatchTable)
        {
            if (eventAny.TypeUrl.EndsWith(suffix))
            {
                var unpackMethod = typeof(Any).GetMethod("Unpack")!.MakeGenericMethod(eventType);
                var evt = unpackMethod.Invoke(eventAny, null);
                var result = method.Invoke(this, new[] { evt });
                return result as Angzarr.Projection ?? new Angzarr.Projection();
            }
        }
        return new Angzarr.Projection();
    }

    /// <summary>
    /// Build a component descriptor for topology discovery.
    /// </summary>
    public Descriptor Descriptor()
    {
        var dispatchTable = _dispatchTables[GetType()];
        return new Descriptor(
            Name,
            ComponentTypes.Projector,
            new List<TargetDesc> { new(InputDomain, dispatchTable.Keys.ToList()) });
    }
}
