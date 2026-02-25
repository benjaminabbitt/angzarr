namespace Angzarr.Client.Router;

/// <summary>
/// Handler for a single domain's events in a projector.
///
/// Projectors consume events and produce external output (read models,
/// caches, external systems).
///
/// Example:
/// <code>
/// public class PlayerProjectorHandler : IProjectorDomainHandler
/// {
///     public IReadOnlyList&lt;string&gt; EventTypes() =>
///         new[] { "PlayerRegistered", "FundsDeposited" };
///
///     public Angzarr.Projection Project(Angzarr.EventBook events)
///     {
///         // Update external read model
///         return new Angzarr.Projection { Projector = "player-projector" };
///     }
/// }
/// </code>
/// </summary>
public interface IProjectorDomainHandler
{
    /// <summary>
    /// Event type suffixes this handler processes.
    /// </summary>
    IReadOnlyList<string> EventTypes();

    /// <summary>
    /// Project events to external output.
    /// </summary>
    /// <param name="events">Event book to project.</param>
    /// <returns>Projection result.</returns>
    Angzarr.Projection Project(Angzarr.EventBook events);
}
