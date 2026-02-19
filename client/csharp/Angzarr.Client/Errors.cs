namespace Angzarr.Client;

/// <summary>
/// Base exception for all Angzarr client errors.
/// </summary>
public class ClientError : Exception
{
    public ClientError(string message) : base(message) { }
    public ClientError(string message, Exception inner) : base(message, inner) { }

    /// <summary>
    /// Returns true if this is a "not found" error.
    /// </summary>
    public virtual bool IsNotFound() => false;

    /// <summary>
    /// Returns true if this is a "precondition failed" error.
    /// </summary>
    public virtual bool IsPreconditionFailed() => false;

    /// <summary>
    /// Returns true if this is an "invalid argument" error.
    /// </summary>
    public virtual bool IsInvalidArgument() => false;

    /// <summary>
    /// Returns true if this is a connection or transport error.
    /// </summary>
    public virtual bool IsConnectionError() => false;
}

/// <summary>
/// Thrown when a command is rejected by business logic.
/// Maps to gRPC FAILED_PRECONDITION status.
/// </summary>
public class CommandRejectedError : ClientError
{
    public CommandRejectedError(string message) : base(message) { }

    public override bool IsPreconditionFailed() => true;
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

    public override bool IsNotFound() => StatusCode == Grpc.Core.StatusCode.NotFound;

    public override bool IsPreconditionFailed() => StatusCode == Grpc.Core.StatusCode.FailedPrecondition;

    public override bool IsInvalidArgument() => StatusCode == Grpc.Core.StatusCode.InvalidArgument;

    public override bool IsConnectionError() => StatusCode == Grpc.Core.StatusCode.Unavailable;
}

/// <summary>
/// Thrown when connection to the server fails.
/// </summary>
public class ConnectionError : ClientError
{
    public ConnectionError(string message) : base(message) { }
    public ConnectionError(string message, Exception inner) : base(message, inner) { }

    public override bool IsConnectionError() => true;
}

/// <summary>
/// Thrown when transport-level errors occur.
/// </summary>
public class TransportError : ClientError
{
    public TransportError(string message) : base(message) { }
    public TransportError(string message, Exception inner) : base(message, inner) { }

    public override bool IsConnectionError() => true;
}

/// <summary>
/// Thrown when an invalid argument is provided.
/// </summary>
public class InvalidArgumentError : ClientError
{
    public InvalidArgumentError(string message) : base(message) { }

    public override bool IsInvalidArgument() => true;
}

/// <summary>
/// Thrown when a timestamp cannot be parsed.
/// </summary>
public class InvalidTimestampError : ClientError
{
    public InvalidTimestampError(string message) : base(message) { }
}
