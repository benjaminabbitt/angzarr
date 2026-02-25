namespace Angzarr.Client.Router;

/// <summary>
/// Response from process manager handlers.
/// Contains commands to send to other aggregates and/or events
/// to persist to the PM's own domain.
/// </summary>
public class ProcessManagerResponse
{
    /// <summary>
    /// Commands to send to other aggregates.
    /// </summary>
    public List<Angzarr.CommandBook> Commands { get; set; } = new();

    /// <summary>
    /// Events to persist to the PM's own domain.
    /// </summary>
    public Angzarr.EventBook? ProcessEvents { get; set; }
}
