using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using System.Reflection;

namespace Angzarr.Client;

/// <summary>
/// Base class for event-sourced aggregates (OO pattern).
/// </summary>
/// <typeparam name="TState">The state type for this aggregate</typeparam>
/// <example>
/// <code>
/// public class PlayerAggregate : Aggregate&lt;PlayerState&gt;
/// {
///     [Applies(typeof(PlayerRegistered))]
///     public void ApplyRegistered(PlayerState state, PlayerRegistered evt)
///     {
///         state.PlayerId = $"player_{evt.Email}";
///         state.DisplayName = evt.DisplayName;
///     }
///
///     [Handles(typeof(RegisterPlayer))]
///     public PlayerRegistered HandleRegister(RegisterPlayer cmd) { ... }
/// }
/// </code>
/// </example>
public abstract class Aggregate<TState> where TState : class, new()
{
    private TState _state = new();
    private readonly Dictionary<System.Type, MethodInfo> _handlers = new();
    private readonly Dictionary<(string domain, string command), MethodInfo> _rejectionHandlers = new();
    private readonly Dictionary<string, (MethodInfo method, System.Type eventType)> _appliers = new();

    protected Aggregate()
    {
        DiscoverHandlers();
    }

    /// <summary>
    /// Apply a single event to state.
    /// Default implementation uses [Applies] attributes to dispatch.
    /// Override this method if you need custom dispatch logic.
    /// </summary>
    protected virtual void ApplyEvent(TState state, Any eventAny)
    {
        var typeUrl = eventAny.TypeUrl;

        foreach (var (suffix, (method, eventType)) in _appliers)
        {
            if (typeUrl.EndsWith(suffix))
            {
                var evt = eventAny.Unpack(eventType);
                method.Invoke(this, new object[] { state, evt });
                return;
            }
        }
        // Unknown event type - silently ignore for forward compatibility
    }

    /// <summary>
    /// Get the current state.
    /// </summary>
    protected TState State => _state;

    /// <summary>
    /// Rehydrate aggregate from event history.
    /// </summary>
    public void Rehydrate(EventBook eventBook)
    {
        _state = new TState();
        foreach (var page in eventBook.Pages)
        {
            ApplyEvent(_state, page.Event);
        }
    }

    /// <summary>
    /// Handle a command using reflection-based dispatch.
    /// </summary>
    public IMessage HandleCommand(IMessage command)
    {
        var commandType = command.GetType();
        if (!_handlers.TryGetValue(commandType, out var handler))
        {
            throw new InvalidOperationException($"No handler for command type: {commandType.Name}");
        }

        try
        {
            var result = handler.Invoke(this, new object[] { command });
            return (IMessage)result!;
        }
        catch (TargetInvocationException ex) when (ex.InnerException is CommandRejectedError)
        {
            throw ex.InnerException;
        }
    }

    /// <summary>
    /// Handle a rejection notification.
    /// </summary>
    public IMessage? HandleRejection(Notification notification)
    {
        var context = CompensationContext.From(notification);
        var key = (context.Domain, context.CommandType);

        if (_rejectionHandlers.TryGetValue(key, out var handler))
        {
            var result = handler.Invoke(this, new object[] { notification });
            return result as IMessage;
        }

        return null; // Delegate to framework
    }

    private void DiscoverHandlers()
    {
        var methods = GetType().GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic);

        foreach (var method in methods)
        {
            // Discover [Handles] methods
            var handlesAttr = method.GetCustomAttribute<HandlesAttribute>();
            if (handlesAttr != null)
            {
                _handlers[handlesAttr.CommandType] = method;
            }

            // Discover [Rejected] methods
            var rejectedAttr = method.GetCustomAttribute<RejectedAttribute>();
            if (rejectedAttr != null)
            {
                _rejectionHandlers[(rejectedAttr.Domain, rejectedAttr.Command)] = method;
            }

            // Discover [Applies] methods
            var appliesAttr = method.GetCustomAttribute<AppliesAttribute>();
            if (appliesAttr != null)
            {
                var suffix = appliesAttr.EventType.Name;
                _appliers[suffix] = (method, appliesAttr.EventType);
            }
        }
    }

    /// <summary>
    /// Create a timestamp for the current time.
    /// </summary>
    protected static Timestamp Now()
    {
        return Timestamp.FromDateTime(DateTime.UtcNow);
    }
}

/// <summary>
/// Extension methods for protobuf Any type.
/// </summary>
public static class AnyExtensions
{
    /// <summary>
    /// Unpack an Any message to a specific type using reflection.
    /// </summary>
    public static IMessage Unpack(this Any any, System.Type messageType)
    {
        var message = (IMessage)Activator.CreateInstance(messageType)!;
        message.MergeFrom(any.Value);
        return message;
    }
}
