namespace Angzarr.Client;

/// <summary>
/// Marks a method as a command handler for the specified command type.
/// The method should return an event or tuple of events.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
public class HandlesAttribute : Attribute
{
    public Type CommandType { get; }

    public HandlesAttribute(Type commandType)
    {
        CommandType = commandType;
    }
}

/// <summary>
/// Marks a method as an event applier for the specified event type.
/// The method should mutate state in place.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
public class AppliesAttribute : Attribute
{
    public Type EventType { get; }

    public AppliesAttribute(Type eventType)
    {
        EventType = eventType;
    }
}

/// <summary>
/// Marks a method as an event handler for sagas or process managers.
/// The method should return a command or tuple of commands.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
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
/// Marks a method as a prepare handler for two-phase saga/PM protocol.
/// The method should return a list of Covers identifying destination aggregates.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
public class PreparesAttribute : Attribute
{
    public Type EventType { get; }

    public PreparesAttribute(Type eventType)
    {
        EventType = eventType;
    }
}

/// <summary>
/// Marks a method as a projector event handler.
/// The method should return a Projection.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
public class ProjectsAttribute : Attribute
{
    public Type EventType { get; }

    public ProjectsAttribute(Type eventType)
    {
        EventType = eventType;
    }
}

/// <summary>
/// Marks a method as a rejection handler for compensation.
/// Called when a command issued by this component is rejected.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
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
/// Marks a method as an upcaster for event version transformation.
/// The method should return the new event version.
/// </summary>
[AttributeUsage(AttributeTargets.Method, AllowMultiple = false)]
public class UpcastsAttribute : Attribute
{
    public Type FromType { get; }
    public Type ToType { get; }

    public UpcastsAttribute(Type fromType, Type toType)
    {
        FromType = fromType;
        ToType = toType;
    }
}
