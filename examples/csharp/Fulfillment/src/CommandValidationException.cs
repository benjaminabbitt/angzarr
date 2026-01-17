using Grpc.Core;

namespace Angzarr.Examples.Fulfillment;

public class CommandValidationException : Exception
{
    public StatusCode StatusCode { get; }

    public CommandValidationException(StatusCode statusCode, string message)
        : base(message)
    {
        StatusCode = statusCode;
    }

    public static CommandValidationException InvalidArgument(string message) =>
        new(StatusCode.InvalidArgument, message);

    public static CommandValidationException FailedPrecondition(string message) =>
        new(StatusCode.FailedPrecondition, message);
}
