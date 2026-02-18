namespace Angzarr.Client;

/// <summary>
/// Marks a method as a command handler.
/// </summary>
[AttributeUsage(AttributeTargets.Method)]
public class HandlesAttribute : Attribute
{
    public Type CommandType { get; }

    public HandlesAttribute(Type commandType)
    {
        CommandType = commandType;
    }
}

/// <summary>
/// Marks a method as a rejection handler for compensation.
/// </summary>
[AttributeUsage(AttributeTargets.Method)]
public class RejectedAttribute : Attribute
{
    public string Domain { get; }
    public string Command { get; }

    public RejectedAttribute(string domain, string command)
    {
        Domain = domain;
        Command = command;
    }
}

/// <summary>
/// Marks a method as a prepare handler for sagas.
/// </summary>
[AttributeUsage(AttributeTargets.Method)]
public class PreparesAttribute : Attribute
{
    public Type EventType { get; }

    public PreparesAttribute(Type eventType)
    {
        EventType = eventType;
    }
}

/// <summary>
/// Marks a method as an event reaction handler for sagas or process managers.
/// </summary>
[AttributeUsage(AttributeTargets.Method)]
public class ReactsToAttribute : Attribute
{
    public Type EventType { get; }
    public string? InputDomain { get; set; }
    public string? OutputDomain { get; set; }

    public ReactsToAttribute(Type eventType)
    {
        EventType = eventType;
    }
}

/// <summary>
/// Marks a method as a projector event handler.
/// </summary>
[AttributeUsage(AttributeTargets.Method)]
public class ProjectsAttribute : Attribute
{
    public Type EventType { get; }

    public ProjectsAttribute(Type eventType)
    {
        EventType = eventType;
    }
}

/// <summary>
/// Marks a method as an event applier for state reconstruction.
/// </summary>
/// <example>
/// <code>
/// [Applies(typeof(PlayerRegistered))]
/// public void ApplyRegistered(PlayerState state, PlayerRegistered evt)
/// {
///     state.PlayerId = $"player_{evt.Email}";
///     state.DisplayName = evt.DisplayName;
/// }
/// </code>
/// </example>
[AttributeUsage(AttributeTargets.Method)]
public class AppliesAttribute : Attribute
{
    public Type EventType { get; }

    public AppliesAttribute(Type eventType)
    {
        EventType = eventType;
    }
}
