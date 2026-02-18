using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Functional router for saga event handling.
/// </summary>
public class EventRouter
{
    private readonly string _name;
    private readonly string _inputDomain;
    private readonly List<(string domain, string command)> _outputs = new();
    private readonly Dictionary<Type, Delegate> _prepareHandlers = new();
    private readonly Dictionary<Type, Delegate> _reactHandlers = new();

    public EventRouter(string name, string inputDomain)
    {
        _name = name;
        _inputDomain = inputDomain;
    }

    /// <summary>
    /// Declare an output domain and command type.
    /// </summary>
    public EventRouter Sends(string domain, string command)
    {
        _outputs.Add((domain, command));
        return this;
    }

    /// <summary>
    /// Register a prepare handler.
    /// </summary>
    public EventRouter Prepare<TEvent>(Func<TEvent, List<Cover>> handler) where TEvent : IMessage
    {
        _prepareHandlers[typeof(TEvent)] = handler;
        return this;
    }

    /// <summary>
    /// Register an event reaction handler.
    /// </summary>
    public EventRouter On<TEvent>(Func<TEvent, List<EventBook>, object> handler) where TEvent : IMessage
    {
        _reactHandlers[typeof(TEvent)] = handler;
        return this;
    }

    /// <summary>
    /// Execute prepare phase.
    /// </summary>
    public List<Cover> DoPrepare(IMessage eventMessage)
    {
        var eventType = eventMessage.GetType();
        if (_prepareHandlers.TryGetValue(eventType, out var handler))
        {
            return (List<Cover>)handler.DynamicInvoke(eventMessage)!;
        }
        return new List<Cover>();
    }

    /// <summary>
    /// Execute handle phase.
    /// </summary>
    public object? DoHandle(IMessage eventMessage, List<EventBook> destinations)
    {
        var eventType = eventMessage.GetType();
        if (_reactHandlers.TryGetValue(eventType, out var handler))
        {
            return handler.DynamicInvoke(eventMessage, destinations);
        }
        return null;
    }

    /// <summary>
    /// Get next sequence number from an event book.
    /// </summary>
    public static uint NextSequence(EventBook? eventBook)
    {
        if (eventBook == null) return 0;
        return eventBook.NextSequence;
    }

    /// <summary>
    /// Pack a command message into Any.
    /// </summary>
    public static Any PackCommand(IMessage command)
    {
        return Any.Pack(command, "type.googleapis.com/");
    }
}
