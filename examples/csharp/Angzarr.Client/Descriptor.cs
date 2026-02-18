namespace Angzarr.Client;

/// <summary>
/// Component type constants for descriptors.
/// </summary>
public static class ComponentTypes
{
    public const string Aggregate = "aggregate";
    public const string Saga = "saga";
    public const string ProcessManager = "process_manager";
    public const string Projector = "projector";
    public const string Upcaster = "upcaster";
}

/// <summary>
/// Describes what a component subscribes to or sends to.
/// </summary>
public record TargetDesc(string Domain, List<string> Types);

/// <summary>
/// Describes a component for topology discovery.
/// </summary>
public record Descriptor(string Name, string ComponentType, List<TargetDesc> Inputs);
