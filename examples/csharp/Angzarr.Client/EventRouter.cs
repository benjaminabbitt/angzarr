using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Unified router for saga event handling.
/// Uses fluent .Domain().On() pattern to register handlers with domain context.
///
/// Example:
/// <code>
/// var router = new EventRouter("saga-table-hand")
///     .Domain("table")
///     .Prepare&lt;HandStarted&gt;(PrepareHandStarted)
///     .On&lt;HandStarted&gt;(HandleHandStarted);
/// </code>
/// </summary>
public class EventRouter
{
    private readonly string _name;
    private string? _currentDomain;
    private readonly Dictionary<Type, Delegate> _prepareHandlers = new();
    private readonly Dictionary<Type, Delegate> _reactHandlers = new();

    public EventRouter(string name)
    {
        _name = name;
    }

    /// <summary>
    /// Set the current domain context for subsequent On() calls.
    /// </summary>
    public EventRouter Domain(string name)
    {
        _currentDomain = name;
        return this;
    }

    /// <summary>
    /// Register a prepare handler.
    /// Must be called after Domain() to set context.
    /// </summary>
    public EventRouter Prepare<TEvent>(Func<TEvent, List<Cover>> handler) where TEvent : IMessage
    {
        if (_currentDomain == null)
            throw new InvalidOperationException("Must call Domain() before Prepare()");
        _prepareHandlers[typeof(TEvent)] = handler;
        return this;
    }

    /// <summary>
    /// Register an event reaction handler.
    /// Must be called after Domain() to set context.
    /// </summary>
    public EventRouter On<TEvent>(Func<TEvent, List<EventBook>, object> handler) where TEvent : IMessage
    {
        if (_currentDomain == null)
            throw new InvalidOperationException("Must call Domain() before On()");
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
