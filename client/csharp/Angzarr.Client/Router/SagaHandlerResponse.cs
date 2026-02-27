namespace Angzarr.Client.Router;

/// <summary>
/// Response from saga handlers.
/// Contains commands to send to other aggregates and/or events/facts
/// to inject directly into target aggregates.
/// </summary>
public class SagaHandlerResponse
{
    /// <summary>
    /// Commands to send to other aggregates.
    /// </summary>
    public List<Angzarr.CommandBook> Commands { get; set; } = new();

    /// <summary>
    /// Events/facts to inject directly into target aggregates.
    /// </summary>
    public List<Angzarr.EventBook> Events { get; set; } = new();

    /// <summary>
    /// Create an empty response.
    /// </summary>
    public static SagaHandlerResponse Empty() => new();

    /// <summary>
    /// Create a response with commands only.
    /// </summary>
    public static SagaHandlerResponse WithCommands(IEnumerable<Angzarr.CommandBook> commands) =>
        new() { Commands = commands.ToList() };

    /// <summary>
    /// Create a response with events only.
    /// </summary>
    public static SagaHandlerResponse WithEvents(IEnumerable<Angzarr.EventBook> events) =>
        new() { Events = events.ToList() };

    /// <summary>
    /// Create a response with both commands and events.
    /// </summary>
    public static SagaHandlerResponse WithBoth(
        IEnumerable<Angzarr.CommandBook> commands,
        IEnumerable<Angzarr.EventBook> events
    ) => new() { Commands = commands.ToList(), Events = events.ToList() };
}
