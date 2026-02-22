"""Error types for the Angzarr client library."""

from typing import Optional

import grpc
from inspect import signature as _mutmut_signature
from typing import Annotated
from typing import Callable
from typing import ClassVar


MutantDict = Annotated[dict[str, Callable], "Mutant"]


def _mutmut_trampoline(orig, mutants, call_args, call_kwargs, self_arg=None):
    """Forward call to original or mutated function, depending on the environment"""
    import os

    mutant_under_test = os.environ["MUTANT_UNDER_TEST"]
    if mutant_under_test == "fail":
        from mutmut.__main__ import MutmutProgrammaticFailException

        raise MutmutProgrammaticFailException("Failed programmatically")
    elif mutant_under_test == "stats":
        from mutmut.__main__ import record_trampoline_hit

        record_trampoline_hit(orig.__module__ + "." + orig.__name__)
        result = orig(*call_args, **call_kwargs)
        return result
    prefix = orig.__module__ + "." + orig.__name__ + "__mutmut_"
    if not mutant_under_test.startswith(prefix):
        result = orig(*call_args, **call_kwargs)
        return result
    mutant_name = mutant_under_test.rpartition(".")[-1]
    if self_arg is not None:
        # call to a class method where self is not bound
        result = mutants[mutant_name](self_arg, *call_args, **call_kwargs)
    else:
        result = mutants[mutant_name](*call_args, **call_kwargs)
    return result


class ClientError(Exception):
    """Base class for client errors."""

    def xǁClientErrorǁ__init____mutmut_orig(
        self, message: str, cause: Optional[Exception] = None
    ):
        super().__init__(message)
        self.message = message
        self.cause = cause

    def xǁClientErrorǁ__init____mutmut_1(
        self, message: str, cause: Optional[Exception] = None
    ):
        super().__init__(None)
        self.message = message
        self.cause = cause

    def xǁClientErrorǁ__init____mutmut_2(
        self, message: str, cause: Optional[Exception] = None
    ):
        super().__init__(message)
        self.message = None
        self.cause = cause

    def xǁClientErrorǁ__init____mutmut_3(
        self, message: str, cause: Optional[Exception] = None
    ):
        super().__init__(message)
        self.message = message
        self.cause = None

    xǁClientErrorǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁClientErrorǁ__init____mutmut_1": xǁClientErrorǁ__init____mutmut_1,
        "xǁClientErrorǁ__init____mutmut_2": xǁClientErrorǁ__init____mutmut_2,
        "xǁClientErrorǁ__init____mutmut_3": xǁClientErrorǁ__init____mutmut_3,
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁClientErrorǁ__init____mutmut_orig"),
            object.__getattribute__(self, "xǁClientErrorǁ__init____mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(xǁClientErrorǁ__init____mutmut_orig)
    xǁClientErrorǁ__init____mutmut_orig.__name__ = "xǁClientErrorǁ__init__"

    def __str__(self) -> str:
        if self.cause:
            return f"{self.message}: {self.cause}"
        return self.message


class ConnectionError(ClientError):
    """Failed to establish connection to the server."""

    def xǁConnectionErrorǁ__init____mutmut_orig(self, message: str):
        super().__init__(f"connection failed: {message}")

    def xǁConnectionErrorǁ__init____mutmut_1(self, message: str):
        super().__init__(None)

    xǁConnectionErrorǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁConnectionErrorǁ__init____mutmut_1": xǁConnectionErrorǁ__init____mutmut_1
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁConnectionErrorǁ__init____mutmut_orig"),
            object.__getattribute__(self, "xǁConnectionErrorǁ__init____mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(xǁConnectionErrorǁ__init____mutmut_orig)
    xǁConnectionErrorǁ__init____mutmut_orig.__name__ = "xǁConnectionErrorǁ__init__"


class TransportError(ClientError):
    """Transport-level error."""

    def xǁTransportErrorǁ__init____mutmut_orig(self, cause: Exception):
        super().__init__("transport error", cause)

    def xǁTransportErrorǁ__init____mutmut_1(self, cause: Exception):
        super().__init__(None, cause)

    def xǁTransportErrorǁ__init____mutmut_2(self, cause: Exception):
        super().__init__("transport error", None)

    def xǁTransportErrorǁ__init____mutmut_3(self, cause: Exception):
        super().__init__(cause)

    def xǁTransportErrorǁ__init____mutmut_4(self, cause: Exception):
        super().__init__(
            "transport error",
        )

    def xǁTransportErrorǁ__init____mutmut_5(self, cause: Exception):
        super().__init__("XXtransport errorXX", cause)

    def xǁTransportErrorǁ__init____mutmut_6(self, cause: Exception):
        super().__init__("TRANSPORT ERROR", cause)

    xǁTransportErrorǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁTransportErrorǁ__init____mutmut_1": xǁTransportErrorǁ__init____mutmut_1,
        "xǁTransportErrorǁ__init____mutmut_2": xǁTransportErrorǁ__init____mutmut_2,
        "xǁTransportErrorǁ__init____mutmut_3": xǁTransportErrorǁ__init____mutmut_3,
        "xǁTransportErrorǁ__init____mutmut_4": xǁTransportErrorǁ__init____mutmut_4,
        "xǁTransportErrorǁ__init____mutmut_5": xǁTransportErrorǁ__init____mutmut_5,
        "xǁTransportErrorǁ__init____mutmut_6": xǁTransportErrorǁ__init____mutmut_6,
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁTransportErrorǁ__init____mutmut_orig"),
            object.__getattribute__(self, "xǁTransportErrorǁ__init____mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(xǁTransportErrorǁ__init____mutmut_orig)
    xǁTransportErrorǁ__init____mutmut_orig.__name__ = "xǁTransportErrorǁ__init__"


class GRPCError(ClientError):
    """gRPC error from the server."""

    def xǁGRPCErrorǁ__init____mutmut_orig(self, cause: grpc.RpcError):
        super().__init__("grpc error", cause)
        self._rpc_error = cause

    def xǁGRPCErrorǁ__init____mutmut_1(self, cause: grpc.RpcError):
        super().__init__(None, cause)
        self._rpc_error = cause

    def xǁGRPCErrorǁ__init____mutmut_2(self, cause: grpc.RpcError):
        super().__init__("grpc error", None)
        self._rpc_error = cause

    def xǁGRPCErrorǁ__init____mutmut_3(self, cause: grpc.RpcError):
        super().__init__(cause)
        self._rpc_error = cause

    def xǁGRPCErrorǁ__init____mutmut_4(self, cause: grpc.RpcError):
        super().__init__(
            "grpc error",
        )
        self._rpc_error = cause

    def xǁGRPCErrorǁ__init____mutmut_5(self, cause: grpc.RpcError):
        super().__init__("XXgrpc errorXX", cause)
        self._rpc_error = cause

    def xǁGRPCErrorǁ__init____mutmut_6(self, cause: grpc.RpcError):
        super().__init__("GRPC ERROR", cause)
        self._rpc_error = cause

    def xǁGRPCErrorǁ__init____mutmut_7(self, cause: grpc.RpcError):
        super().__init__("grpc error", cause)
        self._rpc_error = None

    xǁGRPCErrorǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁGRPCErrorǁ__init____mutmut_1": xǁGRPCErrorǁ__init____mutmut_1,
        "xǁGRPCErrorǁ__init____mutmut_2": xǁGRPCErrorǁ__init____mutmut_2,
        "xǁGRPCErrorǁ__init____mutmut_3": xǁGRPCErrorǁ__init____mutmut_3,
        "xǁGRPCErrorǁ__init____mutmut_4": xǁGRPCErrorǁ__init____mutmut_4,
        "xǁGRPCErrorǁ__init____mutmut_5": xǁGRPCErrorǁ__init____mutmut_5,
        "xǁGRPCErrorǁ__init____mutmut_6": xǁGRPCErrorǁ__init____mutmut_6,
        "xǁGRPCErrorǁ__init____mutmut_7": xǁGRPCErrorǁ__init____mutmut_7,
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁGRPCErrorǁ__init____mutmut_orig"),
            object.__getattribute__(self, "xǁGRPCErrorǁ__init____mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(xǁGRPCErrorǁ__init____mutmut_orig)
    xǁGRPCErrorǁ__init____mutmut_orig.__name__ = "xǁGRPCErrorǁ__init__"

    @property
    def code(self) -> grpc.StatusCode:
        """Return the gRPC status code."""
        return self._rpc_error.code()

    @property
    def details(self) -> str:
        """Return the error details."""
        return self._rpc_error.details()

    def xǁGRPCErrorǁis_not_found__mutmut_orig(self) -> bool:
        """Return True if this is a NOT_FOUND error."""
        return self.code == grpc.StatusCode.NOT_FOUND

    def xǁGRPCErrorǁis_not_found__mutmut_1(self) -> bool:
        """Return True if this is a NOT_FOUND error."""
        return self.code != grpc.StatusCode.NOT_FOUND

    xǁGRPCErrorǁis_not_found__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁGRPCErrorǁis_not_found__mutmut_1": xǁGRPCErrorǁis_not_found__mutmut_1
    }

    def is_not_found(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁGRPCErrorǁis_not_found__mutmut_orig"),
            object.__getattribute__(self, "xǁGRPCErrorǁis_not_found__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    is_not_found.__signature__ = _mutmut_signature(
        xǁGRPCErrorǁis_not_found__mutmut_orig
    )
    xǁGRPCErrorǁis_not_found__mutmut_orig.__name__ = "xǁGRPCErrorǁis_not_found"

    def xǁGRPCErrorǁis_precondition_failed__mutmut_orig(self) -> bool:
        """Return True if this is a FAILED_PRECONDITION error."""
        return self.code == grpc.StatusCode.FAILED_PRECONDITION

    def xǁGRPCErrorǁis_precondition_failed__mutmut_1(self) -> bool:
        """Return True if this is a FAILED_PRECONDITION error."""
        return self.code != grpc.StatusCode.FAILED_PRECONDITION

    xǁGRPCErrorǁis_precondition_failed__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁGRPCErrorǁis_precondition_failed__mutmut_1": xǁGRPCErrorǁis_precondition_failed__mutmut_1
    }

    def is_precondition_failed(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(
                self, "xǁGRPCErrorǁis_precondition_failed__mutmut_orig"
            ),
            object.__getattribute__(
                self, "xǁGRPCErrorǁis_precondition_failed__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    is_precondition_failed.__signature__ = _mutmut_signature(
        xǁGRPCErrorǁis_precondition_failed__mutmut_orig
    )
    xǁGRPCErrorǁis_precondition_failed__mutmut_orig.__name__ = (
        "xǁGRPCErrorǁis_precondition_failed"
    )

    def xǁGRPCErrorǁis_invalid_argument__mutmut_orig(self) -> bool:
        """Return True if this is an INVALID_ARGUMENT error."""
        return self.code == grpc.StatusCode.INVALID_ARGUMENT

    def xǁGRPCErrorǁis_invalid_argument__mutmut_1(self) -> bool:
        """Return True if this is an INVALID_ARGUMENT error."""
        return self.code != grpc.StatusCode.INVALID_ARGUMENT

    xǁGRPCErrorǁis_invalid_argument__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁGRPCErrorǁis_invalid_argument__mutmut_1": xǁGRPCErrorǁis_invalid_argument__mutmut_1
    }

    def is_invalid_argument(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(
                self, "xǁGRPCErrorǁis_invalid_argument__mutmut_orig"
            ),
            object.__getattribute__(
                self, "xǁGRPCErrorǁis_invalid_argument__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    is_invalid_argument.__signature__ = _mutmut_signature(
        xǁGRPCErrorǁis_invalid_argument__mutmut_orig
    )
    xǁGRPCErrorǁis_invalid_argument__mutmut_orig.__name__ = (
        "xǁGRPCErrorǁis_invalid_argument"
    )


class InvalidArgumentError(ClientError):
    """Invalid argument provided by caller."""

    def xǁInvalidArgumentErrorǁ__init____mutmut_orig(self, message: str):
        super().__init__(f"invalid argument: {message}")

    def xǁInvalidArgumentErrorǁ__init____mutmut_1(self, message: str):
        super().__init__(None)

    xǁInvalidArgumentErrorǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁInvalidArgumentErrorǁ__init____mutmut_1": xǁInvalidArgumentErrorǁ__init____mutmut_1
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(
                self, "xǁInvalidArgumentErrorǁ__init____mutmut_orig"
            ),
            object.__getattribute__(
                self, "xǁInvalidArgumentErrorǁ__init____mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(
        xǁInvalidArgumentErrorǁ__init____mutmut_orig
    )
    xǁInvalidArgumentErrorǁ__init____mutmut_orig.__name__ = (
        "xǁInvalidArgumentErrorǁ__init__"
    )


class InvalidTimestampError(ClientError):
    """Failed to parse timestamp."""

    def xǁInvalidTimestampErrorǁ__init____mutmut_orig(self, message: str):
        super().__init__(f"invalid timestamp: {message}")

    def xǁInvalidTimestampErrorǁ__init____mutmut_1(self, message: str):
        super().__init__(None)

    xǁInvalidTimestampErrorǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁInvalidTimestampErrorǁ__init____mutmut_1": xǁInvalidTimestampErrorǁ__init____mutmut_1
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(
                self, "xǁInvalidTimestampErrorǁ__init____mutmut_orig"
            ),
            object.__getattribute__(
                self, "xǁInvalidTimestampErrorǁ__init____mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(
        xǁInvalidTimestampErrorǁ__init____mutmut_orig
    )
    xǁInvalidTimestampErrorǁ__init____mutmut_orig.__name__ = (
        "xǁInvalidTimestampErrorǁ__init__"
    )
