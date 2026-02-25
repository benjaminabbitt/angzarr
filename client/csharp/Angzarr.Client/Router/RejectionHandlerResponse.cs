namespace Angzarr.Client;

/// <summary>
/// Response from rejection handlers - can emit events AND/OR notification.
/// </summary>
public class RejectionHandlerResponse
{
    /// <summary>
    /// Events to persist to own state (compensation).
    /// </summary>
    public Angzarr.EventBook? Events { get; set; }

    /// <summary>
    /// Notification to forward upstream (rejection propagation).
    /// </summary>
    public Angzarr.Notification? Notification { get; set; }
}
