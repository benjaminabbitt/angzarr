using Angzarr.Client;
using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client.Router;

/// <summary>
/// Handler for a single domain's commands.
///
/// Command handlers receive commands and emit events. They maintain state
/// that is rebuilt from events using a StateRouter.
///
/// Example:
/// <code>
/// public class PlayerHandler : ICommandHandlerDomainHandler&lt;PlayerState&gt;
/// {
///     private readonly StateRouter&lt;PlayerState&gt; _stateRouter;
///
///     public PlayerHandler()
///     {
///         _stateRouter = new StateRouter&lt;PlayerState&gt;()
///             .On&lt;PlayerRegistered&gt;((state, evt) => state.Exists = true)
///             .On&lt;FundsDeposited&gt;((state, evt) => state.Bankroll += evt.Amount);
///     }
///
///     public IReadOnlyList&lt;string&gt; CommandTypes() =>
///         new[] { "RegisterPlayer", "DepositFunds" };
///
///     public StateRouter&lt;TState&gt; StateRouter() => _stateRouter;
///
///     public Angzarr.EventBook Handle(
///         Angzarr.CommandBook cmd,
///         Any payload,
///         PlayerState state,
///         int seq)
///     {
///         if (payload.TypeUrl.EndsWith("RegisterPlayer"))
///             return HandleRegister(cmd, payload, state, seq);
///         if (payload.TypeUrl.EndsWith("DepositFunds"))
///             return HandleDeposit(cmd, payload, state, seq);
///         throw new CommandRejectedError($"Unknown command: {payload.TypeUrl}");
///     }
/// }
/// </code>
/// </summary>
/// <typeparam name="TState">The state type for this command handler.</typeparam>
public interface ICommandHandlerDomainHandler<TState>
    where TState : new()
{
    /// <summary>
    /// Command type suffixes this handler processes.
    /// Used for subscription derivation and routing.
    /// </summary>
    IReadOnlyList<string> CommandTypes();

    /// <summary>
    /// Get the state router for rebuilding state from events.
    /// </summary>
    StateRouter<TState> StateRouter();

    /// <summary>
    /// Rebuild state from events.
    /// Default implementation uses StateRouter().WithEventBook().
    /// </summary>
    TState Rebuild(Angzarr.EventBook? events)
    {
        return StateRouter().WithEventBook(events);
    }

    /// <summary>
    /// Handle a command and return resulting events.
    /// The handler should dispatch internally based on payload.TypeUrl.
    /// </summary>
    /// <param name="cmd">The command book containing metadata.</param>
    /// <param name="payload">The command payload as Any.</param>
    /// <param name="state">Current state.</param>
    /// <param name="seq">Next sequence number for events.</param>
    /// <returns>EventBook containing resulting events.</returns>
    Angzarr.EventBook Handle(Angzarr.CommandBook cmd, Any payload, TState state, int seq);

    /// <summary>
    /// Handle a rejection notification.
    /// Called when a command issued by a saga/PM targeting this
    /// domain was rejected. Override to provide custom compensation logic.
    /// Default implementation returns an empty response (framework handles).
    /// </summary>
    /// <param name="notification">The rejection notification.</param>
    /// <param name="state">Current state.</param>
    /// <param name="targetDomain">Domain of the rejected command.</param>
    /// <param name="targetCommand">Command type that was rejected.</param>
    /// <returns>Response with optional compensation events or notification.</returns>
    RejectionHandlerResponse OnRejected(
        Angzarr.Notification notification,
        TState state,
        string targetDomain,
        string targetCommand
    )
    {
        return new RejectionHandlerResponse();
    }
}
