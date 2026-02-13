"""Error types for the Angzarr client library."""

from typing import Optional

import grpc


class ClientError(Exception):
    """Base class for client errors."""

    def __init__(self, message: str, cause: Optional[Exception] = None):
        super().__init__(message)
        self.message = message
        self.cause = cause

    def __str__(self) -> str:
        if self.cause:
            return f"{self.message}: {self.cause}"
        return self.message


class ConnectionError(ClientError):
    """Failed to establish connection to the server."""

    def __init__(self, message: str):
        super().__init__(f"connection failed: {message}")


class TransportError(ClientError):
    """Transport-level error."""

    def __init__(self, cause: Exception):
        super().__init__("transport error", cause)


class GRPCError(ClientError):
    """gRPC error from the server."""

    def __init__(self, cause: grpc.RpcError):
        super().__init__("grpc error", cause)
        self._rpc_error = cause

    @property
    def code(self) -> grpc.StatusCode:
        """Return the gRPC status code."""
        return self._rpc_error.code()

    @property
    def details(self) -> str:
        """Return the error details."""
        return self._rpc_error.details()

    def is_not_found(self) -> bool:
        """Return True if this is a NOT_FOUND error."""
        return self.code == grpc.StatusCode.NOT_FOUND

    def is_precondition_failed(self) -> bool:
        """Return True if this is a FAILED_PRECONDITION error."""
        return self.code == grpc.StatusCode.FAILED_PRECONDITION

    def is_invalid_argument(self) -> bool:
        """Return True if this is an INVALID_ARGUMENT error."""
        return self.code == grpc.StatusCode.INVALID_ARGUMENT


class InvalidArgumentError(ClientError):
    """Invalid argument provided by caller."""

    def __init__(self, message: str):
        super().__init__(f"invalid argument: {message}")


class InvalidTimestampError(ClientError):
    """Failed to parse timestamp."""

    def __init__(self, message: str):
        super().__init__(f"invalid timestamp: {message}")


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""
