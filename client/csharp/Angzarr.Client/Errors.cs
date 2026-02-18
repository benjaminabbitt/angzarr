namespace Angzarr.Client;

/// <summary>
/// Base exception for all Angzarr client errors.
/// </summary>
public class ClientError : Exception
{
    public ClientError(string message) : base(message) { }
    public ClientError(string message, Exception inner) : base(message, inner) { }
}

/// <summary>
/// Thrown when a command is rejected by business logic.
/// Maps to gRPC FAILED_PRECONDITION status.
/// </summary>
public class CommandRejectedError : ClientError
{
    public CommandRejectedError(string message) : base(message) { }
}

/// <summary>
/// Thrown when a gRPC call fails.
/// </summary>
public class GrpcError : ClientError
{
    public Grpc.Core.StatusCode StatusCode { get; }

    public GrpcError(string message, Grpc.Core.StatusCode statusCode)
        : base(message)
    {
        StatusCode = statusCode;
    }
}

/// <summary>
/// Thrown when connection to the server fails.
/// </summary>
public class ConnectionError : ClientError
{
    public ConnectionError(string message) : base(message) { }
    public ConnectionError(string message, Exception inner) : base(message, inner) { }
}

/// <summary>
/// Thrown when transport-level errors occur.
/// </summary>
public class TransportError : ClientError
{
    public TransportError(string message) : base(message) { }
    public TransportError(string message, Exception inner) : base(message, inner) { }
}

/// <summary>
/// Thrown when an invalid argument is provided.
/// </summary>
public class InvalidArgumentError : ClientError
{
    public InvalidArgumentError(string message) : base(message) { }
}

/// <summary>
/// Thrown when a timestamp cannot be parsed.
/// </summary>
public class InvalidTimestampError : ClientError
{
    public InvalidTimestampError(string message) : base(message) { }
}
