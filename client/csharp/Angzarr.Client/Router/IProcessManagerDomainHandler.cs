using Angzarr.Client;
using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client.Router;

/// <summary>
/// Handler for a single domain's events in a process manager.
///
/// Process managers correlate events across multiple domains and maintain
/// their own state. Each domain gets its own handler, but they all share
/// the same PM state type.
///
/// Example:
/// <code>
/// public class OrderPmHandler : IProcessManagerDomainHandler&lt;HandFlowState&gt;
/// {
///     public IReadOnlyList&lt;string&gt; EventTypes() =>
///         new[] { "OrderCreated" };
///
///     public IReadOnlyList&lt;Angzarr.Cover&gt; Prepare(
///         Angzarr.EventBook trigger,
///         HandFlowState state,
///         Any eventPayload)
///     {
///         // Declare needed destinations
///         return new List&lt;Angzarr.Cover&gt;();
///     }
///
///     public ProcessManagerResponse Handle(
///         Angzarr.EventBook trigger,
///         HandFlowState state,
///         Any eventPayload,
///         IReadOnlyList&lt;Angzarr.EventBook&gt; destinations)
///     {
///         // Process event, emit commands and/or PM events
///         return new ProcessManagerResponse();
///     }
/// }
/// </code>
/// </summary>
/// <typeparam name="TState">The PM state type.</typeparam>
public interface IProcessManagerDomainHandler<TState>
{
    /// <summary>
    /// Event type suffixes this handler processes.
    /// </summary>
    IReadOnlyList<string> EventTypes();

    /// <summary>
    /// Prepare phase - declare destination covers needed.
    /// </summary>
    /// <param name="trigger">The triggering event book.</param>
    /// <param name="state">Current PM state.</param>
    /// <param name="eventPayload">The event payload as Any.</param>
    /// <returns>List of covers identifying needed destination aggregates.</returns>
    IReadOnlyList<Angzarr.Cover> Prepare(Angzarr.EventBook trigger, TState state, Any eventPayload);

    /// <summary>
    /// Handle phase - produce commands and PM events.
    /// </summary>
    /// <param name="trigger">The triggering event book.</param>
    /// <param name="state">Current PM state.</param>
    /// <param name="eventPayload">The event payload as Any.</param>
    /// <param name="destinations">Fetched destination aggregate states.</param>
    /// <returns>Response containing commands and/or PM events.</returns>
    ProcessManagerResponse Handle(
        Angzarr.EventBook trigger,
        TState state,
        Any eventPayload,
        IReadOnlyList<Angzarr.EventBook> destinations
    );

    /// <summary>
    /// Handle a rejection notification.
    /// Called when a PM-issued command was rejected. Override to provide
    /// custom compensation logic.
    /// </summary>
    /// <param name="notification">The rejection notification.</param>
    /// <param name="state">Current PM state.</param>
    /// <param name="targetDomain">Domain of the rejected command.</param>
    /// <param name="targetCommand">Command type that was rejected.</param>
    /// <returns>Response with optional compensation events.</returns>
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
