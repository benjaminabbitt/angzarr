using Grpc.Core;

namespace Angzarr.Client;

/// <summary>
/// Exception thrown when a command is rejected by the aggregate.
/// </summary>
public class CommandRejectedError : Exception
{
    public StatusCode StatusCode { get; }

    public CommandRejectedError(string message, StatusCode statusCode = StatusCode.Unknown)
        : base(message)
    {
        StatusCode = statusCode;
    }

    public static CommandRejectedError PreconditionFailed(string message)
    {
        return new CommandRejectedError(message, StatusCode.FailedPrecondition);
    }

    public static CommandRejectedError InvalidArgument(string message)
    {
        return new CommandRejectedError(message, StatusCode.InvalidArgument);
    }

    public static CommandRejectedError NotFound(string message)
    {
        return new CommandRejectedError(message, StatusCode.NotFound);
    }
}
