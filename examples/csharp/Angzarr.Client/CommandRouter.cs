using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;
using Angzarr;
using Type = System.Type;

namespace Angzarr.Client;

/// <summary>
/// Functional router for command handling.
/// </summary>
public class CommandRouter
{
    private readonly string _domain;
    private readonly Func<EventBook, object> _stateBuilder;
    private readonly Dictionary<Type, Delegate> _handlers = new();
    private readonly Dictionary<(string, string), Delegate> _rejectionHandlers = new();

    public CommandRouter(string domain, Func<EventBook, object> stateBuilder)
    {
        _domain = domain;
        _stateBuilder = stateBuilder;
    }

    /// <summary>
    /// Register a command handler.
    /// </summary>
    public CommandRouter On<TCommand>(Func<TCommand, object, IMessage> handler) where TCommand : IMessage
    {
        _handlers[typeof(TCommand)] = handler;
        return this;
    }

    /// <summary>
    /// Register a rejection handler.
    /// </summary>
    public CommandRouter OnRejected(string domain, string command, Func<Notification, object, IMessage?> handler)
    {
        _rejectionHandlers[(domain, command)] = handler;
        return this;
    }

    /// <summary>
    /// Handle a command.
    /// </summary>
    public IMessage Handle(IMessage command, EventBook eventBook)
    {
        var commandType = command.GetType();
        if (!_handlers.TryGetValue(commandType, out var handler))
        {
            throw new InvalidOperationException($"No handler for command: {commandType.Name}");
        }

        var state = _stateBuilder(eventBook);
        var result = handler.DynamicInvoke(command, state);
        return (IMessage)result!;
    }

    /// <summary>
    /// Handle a rejection.
    /// </summary>
    public IMessage? HandleRejection(Notification notification, EventBook eventBook)
    {
        var context = CompensationContext.From(notification);
        var key = (context.Domain, context.CommandType);

        if (_rejectionHandlers.TryGetValue(key, out var handler))
        {
            var state = _stateBuilder(eventBook);
            return handler.DynamicInvoke(notification, state) as IMessage;
        }

        return null;
    }
}
