using Google.Protobuf;
using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client.Router;

/// <summary>
/// Router for command handler components (commands -> events, single domain).
///
/// Domain is set at construction time. No Domain() method exists,
/// enforcing single-domain constraint at compile time.
///
/// Example:
/// <code>
/// var router = new CommandHandlerRouter&lt;PlayerState, PlayerHandler&gt;(
///     "player",
///     "player",
///     new PlayerHandler()
/// );
///
/// // Get subscriptions for framework registration
/// var subs = router.Subscriptions();
///
/// // Dispatch commands
/// var response = router.Dispatch(contextualCommand);
/// </code>
/// </summary>
/// <typeparam name="TState">The state type for this command handler.</typeparam>
/// <typeparam name="THandler">The handler implementation type.</typeparam>
public class CommandHandlerRouter<TState, THandler>
    where TState : new()
    where THandler : ICommandHandlerDomainHandler<TState>
{
    private readonly string _name;
    private readonly string _domain;
    private readonly THandler _handler;

    /// <summary>
    /// Create a new command handler router.
    /// Command handlers receive commands and emit events. Single domain enforced at construction.
    /// </summary>
    /// <param name="name">Component name.</param>
    /// <param name="domain">Domain this command handler belongs to.</param>
    /// <param name="handler">Handler implementation.</param>
    public CommandHandlerRouter(string name, string domain, THandler handler)
    {
        _name = name;
        _domain = domain;
        _handler = handler;
    }

    /// <summary>
    /// Get the router name.
    /// </summary>
    public string Name => _name;

    /// <summary>
    /// Get the domain.
    /// </summary>
    public string Domain => _domain;

    /// <summary>
    /// Get command types from the handler.
    /// </summary>
    public IReadOnlyList<string> CommandTypes() => _handler.CommandTypes();

    /// <summary>
    /// Get subscriptions for this command handler.
    /// Returns list of (domain, command types) tuples.
    /// </summary>
    public IReadOnlyList<(string Domain, IReadOnlyList<string> Types)> Subscriptions()
    {
        return new[] { (_domain, _handler.CommandTypes()) };
    }

    /// <summary>
    /// Rebuild state from events using the handler's state router.
    /// </summary>
    public TState RebuildState(Angzarr.EventBook? events)
    {
        return _handler.Rebuild(events);
    }

    /// <summary>
    /// Dispatch a contextual command to the handler.
    /// </summary>
    /// <param name="cmd">The contextual command containing command book and prior events.</param>
    /// <returns>Business response with events or rejection.</returns>
    public Angzarr.BusinessResponse Dispatch(Angzarr.ContextualCommand cmd)
    {
        var commandBook = cmd.Command;
        if (commandBook == null)
            throw new InvalidArgumentError("Missing command book");

        if (commandBook.Pages.Count == 0)
            throw new InvalidArgumentError("Missing command page");

        var commandPage = commandBook.Pages[0];
        var commandAny = commandPage.Command;
        if (commandAny == null)
            throw new InvalidArgumentError("Missing command");

        var eventBook = cmd.Events;

        // Rebuild state
        var state = _handler.Rebuild(eventBook);
        var seq = Helpers.NextSequence(eventBook);

        var typeUrl = commandAny.TypeUrl;

        // Check for Notification (rejection/compensation)
        if (typeUrl.EndsWith("Notification"))
        {
            return DispatchNotification(commandAny, state);
        }

        // Execute handler
        var resultBook = _handler.Handle(commandBook, commandAny, state, (int)seq);

        return new Angzarr.BusinessResponse { Events = resultBook };
    }

    private Angzarr.BusinessResponse DispatchNotification(Any commandAny, TState state)
    {
        var notification = commandAny.Unpack<Angzarr.Notification>();

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

        var response = _handler.OnRejected(notification, state, domain, commandSuffix);

        if (response.Events != null)
        {
            return new Angzarr.BusinessResponse { Events = response.Events };
        }

        if (response.Notification != null)
        {
            return new Angzarr.BusinessResponse { Notification = response.Notification };
        }

        return new Angzarr.BusinessResponse
        {
            Revocation = new Angzarr.RevocationResponse
            {
                EmitSystemRevocation = true,
                SendToDeadLetterQueue = false,
                Escalate = false,
                Abort = false,
                Reason = $"Handler returned empty response for {domain}/{commandSuffix}",
            },
        };
    }
}
