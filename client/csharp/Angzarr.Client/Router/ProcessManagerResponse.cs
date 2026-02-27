namespace Angzarr.Client.Router;

/// <summary>
/// Response from process manager handlers.
/// Contains commands to send to other aggregates, events to persist
/// to the PM's own domain, and facts to inject into other aggregates.
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

    /// <summary>
    /// Facts to inject directly into other aggregates.
    /// </summary>
    public List<Angzarr.EventBook> Facts { get; set; } = new();

    /// <summary>
    /// Create an empty response.
    /// </summary>
    public static ProcessManagerResponse Empty() => new();

    /// <summary>
    /// Create a response with commands only.
    /// </summary>
    public static ProcessManagerResponse WithCommands(IEnumerable<Angzarr.CommandBook> commands) =>
        new() { Commands = commands.ToList() };

    /// <summary>
    /// Create a response with process events only.
    /// </summary>
    public static ProcessManagerResponse WithProcessEvents(Angzarr.EventBook processEvents) =>
        new() { ProcessEvents = processEvents };

    /// <summary>
    /// Create a response with facts only.
    /// </summary>
    public static ProcessManagerResponse WithFacts(IEnumerable<Angzarr.EventBook> facts) =>
        new() { Facts = facts.ToList() };

    /// <summary>
    /// Create a response with commands and process events.
    /// </summary>
    public static ProcessManagerResponse WithBoth(
        IEnumerable<Angzarr.CommandBook> commands,
        Angzarr.EventBook processEvents
    ) => new() { Commands = commands.ToList(), ProcessEvents = processEvents };

    /// <summary>
    /// Create a response with all fields.
    /// </summary>
    public static ProcessManagerResponse WithAll(
        IEnumerable<Angzarr.CommandBook> commands,
        Angzarr.EventBook processEvents,
        IEnumerable<Angzarr.EventBook> facts
    ) =>
        new()
        {
            Commands = commands.ToList(),
            ProcessEvents = processEvents,
            Facts = facts.ToList(),
        };
}
