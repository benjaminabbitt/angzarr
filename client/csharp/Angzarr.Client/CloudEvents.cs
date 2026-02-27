using System.Reflection;
using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// CloudEvents projectors transform internal domain events into CloudEvents 1.0 format
/// for external consumption via HTTP webhooks or Kafka.
///
/// OO Pattern (CloudEventsProjector):
/// <code>
/// public class PlayerCloudEventsProjector : CloudEventsProjector
/// {
///     public override string Name => "prj-player-cloudevents";
///     public override string InputDomain => "player";
///
///     [CloudEventsHandler(typeof(PlayerRegistered))]
///     public CloudEvent? OnPlayerRegistered(PlayerRegistered evt) =>
///         new CloudEvent
///         {
///             Type = "com.poker.player.registered",
///             Data = Any.Pack(new PublicPlayerRegistered { DisplayName = evt.DisplayName })
///         };
/// }
/// </code>
///
/// Functional Pattern (CloudEventsRouter):
/// <code>
/// var router = new CloudEventsRouter("prj-player-cloudevents", "player")
///     .On&lt;PlayerRegistered&gt;(evt => new CloudEvent
///     {
///         Type = "com.poker.player.registered",
///         Data = Any.Pack(new PublicPlayerRegistered { DisplayName = evt.DisplayName })
///     });
/// </code>
/// </summary>
public abstract class CloudEventsProjector
{
    private static readonly Dictionary<
        Type,
        Dictionary<string, (MethodInfo Method, Type EventType)>
    > _dispatchTables = new();

    /// <summary>
    /// The name of this projector (e.g., "prj-player-cloudevents").
    /// </summary>
    public abstract string Name { get; }

    /// <summary>
    /// The domain this projector listens to.
    /// </summary>
    public abstract string InputDomain { get; }

    protected CloudEventsProjector()
    {
        EnsureDispatchTableBuilt();
    }

    private void EnsureDispatchTableBuilt()
    {
        var type = GetType();
        if (_dispatchTables.ContainsKey(type))
            return;

        lock (_dispatchTables)
        {
            if (_dispatchTables.ContainsKey(type))
                return;

            var dispatch = new Dictionary<string, (MethodInfo, Type)>();
            foreach (
                var method in type.GetMethods(
                    BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic
                )
            )
            {
                var attr = method.GetCustomAttribute<CloudEventsHandlerAttribute>();
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
    /// Process an EventBook and return CloudEvents.
    /// </summary>
    public static CloudEventsResponse Handle<T>(Angzarr.EventBook source)
        where T : CloudEventsProjector, new()
    {
        var projector = new T();
        return projector.Project(source);
    }

    /// <summary>
    /// Process an EventBook and return CloudEvents.
    /// </summary>
    public CloudEventsResponse Project(Angzarr.EventBook source)
    {
        var response = new CloudEventsResponse();
        var dispatchTable = _dispatchTables[GetType()];

        foreach (var page in source.Pages)
        {
            if (page.Event == null)
                continue;

            foreach (var (suffix, (method, eventType)) in dispatchTable)
            {
                if (page.Event.TypeUrl.EndsWith(suffix))
                {
                    var unpackMethod = typeof(Any)
                        .GetMethod("Unpack")!
                        .MakeGenericMethod(eventType);
                    var evt = unpackMethod.Invoke(page.Event, null);
                    var result = method.Invoke(this, new[] { evt });
                    if (result is CloudEvent ce)
                    {
                        response.Events.Add(ce);
                    }
                    break;
                }
            }
        }

        return response;
    }

    /// <summary>
    /// Handle a projector request, returning a Projection containing CloudEventsResponse.
    /// </summary>
    public Angzarr.Projection HandleRequest(Angzarr.EventBook source)
    {
        var response = Project(source);
        return new Angzarr.Projection
        {
            Projector = Name,
            Projection_ = Any.Pack(response, "type.googleapis.com/"),
        };
    }
}

/// <summary>
/// Attribute to mark a method as a CloudEvents handler.
/// </summary>
[AttributeUsage(AttributeTargets.Method)]
public class CloudEventsHandlerAttribute : Attribute
{
    public Type EventType { get; }

    public CloudEventsHandlerAttribute(Type eventType)
    {
        EventType = eventType;
    }
}

/// <summary>
/// Functional router for CloudEvents projectors.
///
/// Example:
/// <code>
/// var router = new CloudEventsRouter("prj-player-cloudevents", "player")
///     .On&lt;PlayerRegistered&gt;(evt => new CloudEvent
///     {
///         Type = "com.poker.player.registered",
///         Data = Any.Pack(new PublicPlayerRegistered { DisplayName = evt.DisplayName })
///     });
///
/// var response = router.Project(eventBook);
/// </code>
/// </summary>
public class CloudEventsRouter
{
    private readonly string _name;
    private readonly string _inputDomain;
    private readonly List<CloudEventsEntry> _handlers = new();

    private sealed class CloudEventsEntry
    {
        public string Suffix { get; }
        public Type EventType { get; }
        public Delegate Handler { get; }

        public CloudEventsEntry(string suffix, Type eventType, Delegate handler)
        {
            Suffix = suffix;
            EventType = eventType;
            Handler = handler;
        }
    }

    /// <summary>
    /// Create a new CloudEvents router.
    /// </summary>
    /// <param name="name">The projector name.</param>
    /// <param name="inputDomain">The domain this projector listens to.</param>
    public CloudEventsRouter(string name, string inputDomain)
    {
        _name = name;
        _inputDomain = inputDomain;
    }

    /// <summary>
    /// The projector name.
    /// </summary>
    public string Name => _name;

    /// <summary>
    /// The domain this projector listens to.
    /// </summary>
    public string InputDomain => _inputDomain;

    /// <summary>
    /// Register a handler for an event type.
    ///
    /// The handler receives the typed event and returns a CloudEvent or null to skip.
    /// </summary>
    /// <typeparam name="TEvent">The event type to handle.</typeparam>
    /// <param name="handler">Function that transforms event to CloudEvent.</param>
    /// <returns>This router for fluent chaining.</returns>
    public CloudEventsRouter On<TEvent>(Func<TEvent, CloudEvent?> handler)
        where TEvent : IMessage
    {
        var suffix = typeof(TEvent).Name;
        _handlers.Add(new CloudEventsEntry(suffix, typeof(TEvent), handler));
        return this;
    }

    /// <summary>
    /// Register a handler for an event type suffix.
    ///
    /// The handler receives the Any payload and returns a CloudEvent or null to skip.
    /// </summary>
    /// <param name="suffix">The type_url suffix to match.</param>
    /// <param name="handler">Function that transforms Any to CloudEvent.</param>
    /// <returns>This router for fluent chaining.</returns>
    public CloudEventsRouter OnSuffix(string suffix, Func<Any, CloudEvent?> handler)
    {
        _handlers.Add(new CloudEventsEntry(suffix, typeof(Any), handler));
        return this;
    }

    /// <summary>
    /// Process an EventBook and return CloudEvents.
    /// </summary>
    /// <param name="source">The EventBook containing events to process.</param>
    /// <returns>CloudEventsResponse containing produced CloudEvents.</returns>
    public CloudEventsResponse Project(Angzarr.EventBook source)
    {
        var response = new CloudEventsResponse();

        foreach (var page in source.Pages)
        {
            if (page.Event == null)
                continue;

            var typeUrl = page.Event.TypeUrl;

            foreach (var entry in _handlers)
            {
                if (typeUrl.EndsWith(entry.Suffix))
                {
                    CloudEvent? result;

                    if (entry.EventType == typeof(Any))
                    {
                        // Suffix handler - pass Any directly
                        result = ((Func<Any, CloudEvent?>)entry.Handler)(page.Event);
                    }
                    else
                    {
                        // Typed handler - unpack and invoke
                        var unpackMethod = typeof(Any)
                            .GetMethod("Unpack")!
                            .MakeGenericMethod(entry.EventType);
                        var evt = unpackMethod.Invoke(page.Event, null);
                        result = (CloudEvent?)entry.Handler.DynamicInvoke(evt);
                    }

                    if (result != null)
                    {
                        response.Events.Add(result);
                    }
                    break;
                }
            }
        }

        return response;
    }

    /// <summary>
    /// Handle a projector request, returning a Projection containing CloudEventsResponse.
    /// </summary>
    /// <param name="source">The EventBook containing events to process.</param>
    /// <returns>Projection containing CloudEventsResponse.</returns>
    public Angzarr.Projection HandleRequest(Angzarr.EventBook source)
    {
        var response = Project(source);
        return new Angzarr.Projection
        {
            Projector = _name,
            Projection_ = Any.Pack(response, "type.googleapis.com/"),
        };
    }

    /// <summary>
    /// Get the event types this router handles.
    /// </summary>
    /// <returns>List of event type suffixes.</returns>
    public IReadOnlyList<string> EventTypes()
    {
        return _handlers.Select(h => h.Suffix).ToList();
    }

    /// <summary>
    /// Get the subscription configuration for framework registration.
    /// </summary>
    /// <returns>Tuple of (domain, event types).</returns>
    public (string Domain, IReadOnlyList<string> Types) Subscriptions()
    {
        return (_inputDomain, EventTypes());
    }
}
