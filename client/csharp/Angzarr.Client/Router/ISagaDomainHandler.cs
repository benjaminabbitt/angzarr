using Google.Protobuf.WellKnownTypes;

namespace Angzarr.Client.Router;

/// <summary>
/// Handler for a single domain's events in a saga.
///
/// Sagas translate events from one domain into commands for another.
/// They are stateless - each event is processed independently.
///
/// Example:
/// <code>
/// public class OrderSagaHandler : ISagaDomainHandler
/// {
///     public IReadOnlyList&lt;string&gt; EventTypes() =>
///         new[] { "OrderCompleted", "OrderCancelled" };
///
///     public IReadOnlyList&lt;Angzarr.Cover&gt; Prepare(
///         Angzarr.EventBook source,
///         Any eventPayload)
///     {
///         if (eventPayload.TypeUrl.EndsWith("OrderCompleted"))
///             return PrepareOrderCompleted(source, eventPayload);
///         return new List&lt;Angzarr.Cover&gt;();
///     }
///
///     public IReadOnlyList&lt;Angzarr.CommandBook&gt; Execute(
///         Angzarr.EventBook source,
///         Any eventPayload,
///         IReadOnlyList&lt;Angzarr.EventBook&gt; destinations)
///     {
///         if (eventPayload.TypeUrl.EndsWith("OrderCompleted"))
///             return HandleOrderCompleted(source, eventPayload, destinations);
///         return new List&lt;Angzarr.CommandBook&gt;();
///     }
/// }
/// </code>
/// </summary>
public interface ISagaDomainHandler
{
    /// <summary>
    /// Event type suffixes this handler processes.
    /// Used for subscription derivation.
    /// </summary>
    IReadOnlyList<string> EventTypes();

    /// <summary>
    /// Prepare phase - declare destination covers needed.
    /// Called before Execute to fetch destination aggregate state.
    /// </summary>
    /// <param name="source">Source event book.</param>
    /// <param name="eventPayload">The event payload as Any.</param>
    /// <returns>List of covers identifying destination aggregates.</returns>
    IReadOnlyList<Angzarr.Cover> Prepare(Angzarr.EventBook source, Any eventPayload);

    /// <summary>
    /// Execute phase - produce commands.
    /// Called with source event and fetched destination state.
    /// </summary>
    /// <param name="source">Source event book.</param>
    /// <param name="eventPayload">The event payload as Any.</param>
    /// <param name="destinations">Fetched destination aggregate states.</param>
    /// <returns>Commands to send to other aggregates.</returns>
    IReadOnlyList<Angzarr.CommandBook> Execute(
        Angzarr.EventBook source,
        Any eventPayload,
        IReadOnlyList<Angzarr.EventBook> destinations
    );
}
