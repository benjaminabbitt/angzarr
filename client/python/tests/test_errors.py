"""Tests for error types."""

from unittest.mock import Mock

import grpc
import pytest

from angzarr_client.errors import (
    ClientError,
    ConnectionError,
    TransportError,
    GRPCError,
    InvalidArgumentError,
    InvalidTimestampError,
)


class TestClientError:
    """Tests for the ClientError base class."""

    def test_message_only(self) -> None:
        """Error with message only."""
        err = ClientError("something went wrong")
        assert err.message == "something went wrong"
        assert err.cause is None
        assert str(err) == "something went wrong"

    def test_with_cause(self) -> None:
        """Error with underlying cause."""
        cause = ValueError("underlying issue")
        err = ClientError("wrapper", cause)
        assert err.message == "wrapper"
        assert err.cause is cause
        assert str(err) == "wrapper: underlying issue"

    def test_exception_inheritance(self) -> None:
        """ClientError is an Exception."""
        err = ClientError("test")
        assert isinstance(err, Exception)


class TestConnectionError:
    """Tests for ConnectionError."""

    def test_message_formatting(self) -> None:
        """Connection error prefixes message."""
        err = ConnectionError("host unreachable")
        assert str(err) == "connection failed: host unreachable"
        assert err.message == "connection failed: host unreachable"
        assert err.cause is None

    def test_exception_inheritance(self) -> None:
        """ConnectionError is a ClientError."""
        err = ConnectionError("test")
        assert isinstance(err, ClientError)
        assert isinstance(err, Exception)


class TestTransportError:
    """Tests for TransportError."""

    def test_wraps_cause(self) -> None:
        """Transport error wraps an underlying exception."""
        cause = OSError("socket error")
        err = TransportError(cause)
        assert str(err) == "transport error: socket error"
        assert err.cause is cause

    def test_exception_inheritance(self) -> None:
        """TransportError is a ClientError."""
        cause = OSError("test")
        err = TransportError(cause)
        assert isinstance(err, ClientError)


class MockRpcError(grpc.RpcError):
    """Mock RpcError for testing.

    grpc.RpcError itself doesn't have code/details methods - those come
    from grpc.Call. Real gRPC errors inherit from both.
    """

    def __init__(self, code: grpc.StatusCode, details: str = ""):
        super().__init__()
        self._code = code
        self._details = details

    def code(self) -> grpc.StatusCode:
        return self._code

    def details(self) -> str:
        return self._details


class TestGRPCError:
    """Tests for GRPCError."""

    def _mock_rpc_error(
        self, code: grpc.StatusCode, details: str
    ) -> grpc.RpcError:
        """Create a mock RpcError with code() and details() methods."""
        return MockRpcError(code, details)

    def test_wraps_rpc_error(self) -> None:
        """GRPCError wraps an RpcError."""
        rpc_error = self._mock_rpc_error(
            grpc.StatusCode.INTERNAL, "server error"
        )
        err = GRPCError(rpc_error)
        assert "grpc error" in str(err)
        assert err.cause is rpc_error

    def test_code_property(self) -> None:
        """Code property returns the gRPC status code."""
        rpc_error = self._mock_rpc_error(
            grpc.StatusCode.NOT_FOUND, "not found"
        )
        err = GRPCError(rpc_error)
        assert err.code == grpc.StatusCode.NOT_FOUND

    def test_details_property(self) -> None:
        """Details property returns error details."""
        rpc_error = self._mock_rpc_error(
            grpc.StatusCode.INVALID_ARGUMENT, "bad request"
        )
        err = GRPCError(rpc_error)
        assert err.details == "bad request"

    def test_is_not_found_true(self) -> None:
        """is_not_found returns True for NOT_FOUND."""
        rpc_error = self._mock_rpc_error(grpc.StatusCode.NOT_FOUND, "")
        err = GRPCError(rpc_error)
        assert err.is_not_found() is True

    def test_is_not_found_false(self) -> None:
        """is_not_found returns False for other codes."""
        rpc_error = self._mock_rpc_error(grpc.StatusCode.INTERNAL, "")
        err = GRPCError(rpc_error)
        assert err.is_not_found() is False

    def test_is_precondition_failed_true(self) -> None:
        """is_precondition_failed returns True for FAILED_PRECONDITION."""
        rpc_error = self._mock_rpc_error(grpc.StatusCode.FAILED_PRECONDITION, "")
        err = GRPCError(rpc_error)
        assert err.is_precondition_failed() is True

    def test_is_precondition_failed_false(self) -> None:
        """is_precondition_failed returns False for other codes."""
        rpc_error = self._mock_rpc_error(grpc.StatusCode.OK, "")
        err = GRPCError(rpc_error)
        assert err.is_precondition_failed() is False

    def test_is_invalid_argument_true(self) -> None:
        """is_invalid_argument returns True for INVALID_ARGUMENT."""
        rpc_error = self._mock_rpc_error(grpc.StatusCode.INVALID_ARGUMENT, "")
        err = GRPCError(rpc_error)
        assert err.is_invalid_argument() is True

    def test_is_invalid_argument_false(self) -> None:
        """is_invalid_argument returns False for other codes."""
        rpc_error = self._mock_rpc_error(grpc.StatusCode.CANCELLED, "")
        err = GRPCError(rpc_error)
        assert err.is_invalid_argument() is False

    def test_exception_inheritance(self) -> None:
        """GRPCError is a ClientError."""
        rpc_error = self._mock_rpc_error(grpc.StatusCode.OK, "")
        err = GRPCError(rpc_error)
        assert isinstance(err, ClientError)


class TestInvalidArgumentError:
    """Tests for InvalidArgumentError."""

    def test_message_formatting(self) -> None:
        """Invalid argument error prefixes message."""
        err = InvalidArgumentError("missing field")
        assert str(err) == "invalid argument: missing field"
        assert err.cause is None

    def test_exception_inheritance(self) -> None:
        """InvalidArgumentError is a ClientError."""
        err = InvalidArgumentError("test")
        assert isinstance(err, ClientError)


class TestInvalidTimestampError:
    """Tests for InvalidTimestampError."""

    def test_message_formatting(self) -> None:
        """Invalid timestamp error prefixes message."""
        err = InvalidTimestampError("bad format")
        assert str(err) == "invalid timestamp: bad format"
        assert err.cause is None

    def test_exception_inheritance(self) -> None:
        """InvalidTimestampError is a ClientError."""
        err = InvalidTimestampError("test")
        assert isinstance(err, ClientError)
