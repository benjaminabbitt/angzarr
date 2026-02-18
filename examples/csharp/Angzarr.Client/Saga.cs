using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using System.Reflection;

namespace Angzarr.Client;

/// <summary>
/// Base class for sagas (OO pattern).
/// </summary>
public abstract class Saga
{
    private readonly Dictionary<System.Type, MethodInfo> _prepareHandlers = new();
    private readonly Dictionary<System.Type, MethodInfo> _reactHandlers = new();

    protected Saga()
    {
        DiscoverHandlers();
    }

    /// <summary>
    /// Get the saga name.
    /// </summary>
    public abstract string Name { get; }

    /// <summary>
    /// Get the input domain this saga listens to.
    /// </summary>
    public abstract string InputDomain { get; }

    /// <summary>
    /// Get the output domain this saga sends commands to.
    /// </summary>
    public abstract string OutputDomain { get; }

    /// <summary>
    /// Prepare phase - declare destinations needed.
    /// </summary>
    public List<Cover> Prepare(IMessage eventMessage)
    {
        var eventType = eventMessage.GetType();
        if (_prepareHandlers.TryGetValue(eventType, out var handler))
        {
            var result = handler.Invoke(this, new object[] { eventMessage });
            return (List<Cover>)result!;
        }
        return new List<Cover>();
    }

    /// <summary>
    /// Handle phase - process event and produce commands.
    /// </summary>
    public object? Handle(IMessage eventMessage, List<EventBook> destinations)
    {
        var eventType = eventMessage.GetType();
        if (_reactHandlers.TryGetValue(eventType, out var handler))
        {
            return handler.Invoke(this, new object[] { eventMessage, destinations });
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

    private void DiscoverHandlers()
    {
        var methods = GetType().GetMethods(BindingFlags.Instance | BindingFlags.Public | BindingFlags.NonPublic);

        foreach (var method in methods)
        {
            var preparesAttr = method.GetCustomAttribute<PreparesAttribute>();
            if (preparesAttr != null)
            {
                _prepareHandlers[preparesAttr.EventType] = method;
            }

            var reactsAttr = method.GetCustomAttribute<ReactsToAttribute>();
            if (reactsAttr != null)
            {
                _reactHandlers[reactsAttr.EventType] = method;
            }
        }
    }
}
