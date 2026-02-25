namespace Angzarr.Client.Router;

/// <summary>
/// Exception thrown when a command is rejected by business logic.
/// Maps to gRPC FAILED_PRECONDITION status.
///
/// This is an alias for CommandRejectedError for naming consistency
/// with the Rust client implementation.
/// </summary>
public class CommandRejectedException : CommandRejectedError
{
    public CommandRejectedException(string message)
        : base(message) { }

    /// <summary>
    /// Create a new CommandRejectedException with the given reason.
    /// </summary>
    public static CommandRejectedException WithReason(string reason)
    {
        return new CommandRejectedException(reason);
    }
}
