"""Fluent builders for commands and queries."""

from typing import Optional
from uuid import UUID as PyUUID, uuid4

from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.message import Message

from .proto.angzarr import (
    Cover,
    Edition,
    CommandBook,
    CommandPage,
    CommandResponse,
    Query,
    SequenceRange,
    TemporalQuery,
    EventBook,
    EventPage,
)
from .client import AggregateClient, QueryClient
from .helpers import uuid_to_proto, implicit_edition, parse_timestamp
from .errors import InvalidArgumentError
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


class CommandBuilder:
    """Fluent builder for constructing and executing commands."""

    def xǁCommandBuilderǁ__init____mutmut_orig(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = 0
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_1(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = None
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = 0
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_2(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = None
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = 0
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_3(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = None
        self._correlation_id: Optional[str] = None
        self._sequence: int = 0
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_4(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = ""
        self._sequence: int = 0
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_5(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = None
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_6(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = 1
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_7(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = 0
        self._type_url: Optional[str] = ""
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_8(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = 0
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = ""
        self._err: Optional[Exception] = None

    def xǁCommandBuilderǁ__init____mutmut_9(
        self,
        client: AggregateClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._sequence: int = 0
        self._type_url: Optional[str] = None
        self._payload: Optional[bytes] = None
        self._err: Optional[Exception] = ""

    xǁCommandBuilderǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁCommandBuilderǁ__init____mutmut_1": xǁCommandBuilderǁ__init____mutmut_1,
        "xǁCommandBuilderǁ__init____mutmut_2": xǁCommandBuilderǁ__init____mutmut_2,
        "xǁCommandBuilderǁ__init____mutmut_3": xǁCommandBuilderǁ__init____mutmut_3,
        "xǁCommandBuilderǁ__init____mutmut_4": xǁCommandBuilderǁ__init____mutmut_4,
        "xǁCommandBuilderǁ__init____mutmut_5": xǁCommandBuilderǁ__init____mutmut_5,
        "xǁCommandBuilderǁ__init____mutmut_6": xǁCommandBuilderǁ__init____mutmut_6,
        "xǁCommandBuilderǁ__init____mutmut_7": xǁCommandBuilderǁ__init____mutmut_7,
        "xǁCommandBuilderǁ__init____mutmut_8": xǁCommandBuilderǁ__init____mutmut_8,
        "xǁCommandBuilderǁ__init____mutmut_9": xǁCommandBuilderǁ__init____mutmut_9,
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁCommandBuilderǁ__init____mutmut_orig"),
            object.__getattribute__(self, "xǁCommandBuilderǁ__init____mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(xǁCommandBuilderǁ__init____mutmut_orig)
    xǁCommandBuilderǁ__init____mutmut_orig.__name__ = "xǁCommandBuilderǁ__init__"

    def xǁCommandBuilderǁwith_correlation_id__mutmut_orig(
        self, id: str
    ) -> "CommandBuilder":
        """Set the correlation ID for request tracing."""
        self._correlation_id = id
        return self

    def xǁCommandBuilderǁwith_correlation_id__mutmut_1(
        self, id: str
    ) -> "CommandBuilder":
        """Set the correlation ID for request tracing."""
        self._correlation_id = None
        return self

    xǁCommandBuilderǁwith_correlation_id__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁCommandBuilderǁwith_correlation_id__mutmut_1": xǁCommandBuilderǁwith_correlation_id__mutmut_1
    }

    def with_correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(
                self, "xǁCommandBuilderǁwith_correlation_id__mutmut_orig"
            ),
            object.__getattribute__(
                self, "xǁCommandBuilderǁwith_correlation_id__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    with_correlation_id.__signature__ = _mutmut_signature(
        xǁCommandBuilderǁwith_correlation_id__mutmut_orig
    )
    xǁCommandBuilderǁwith_correlation_id__mutmut_orig.__name__ = (
        "xǁCommandBuilderǁwith_correlation_id"
    )

    def xǁCommandBuilderǁwith_sequence__mutmut_orig(self, seq: int) -> "CommandBuilder":
        """Set the expected sequence number for optimistic locking."""
        self._sequence = seq
        return self

    def xǁCommandBuilderǁwith_sequence__mutmut_1(self, seq: int) -> "CommandBuilder":
        """Set the expected sequence number for optimistic locking."""
        self._sequence = None
        return self

    xǁCommandBuilderǁwith_sequence__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁCommandBuilderǁwith_sequence__mutmut_1": xǁCommandBuilderǁwith_sequence__mutmut_1
    }

    def with_sequence(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(
                self, "xǁCommandBuilderǁwith_sequence__mutmut_orig"
            ),
            object.__getattribute__(
                self, "xǁCommandBuilderǁwith_sequence__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    with_sequence.__signature__ = _mutmut_signature(
        xǁCommandBuilderǁwith_sequence__mutmut_orig
    )
    xǁCommandBuilderǁwith_sequence__mutmut_orig.__name__ = (
        "xǁCommandBuilderǁwith_sequence"
    )

    def xǁCommandBuilderǁwith_command__mutmut_orig(
        self, type_url: str, message: Message
    ) -> "CommandBuilder":
        """Set the command type URL and message."""
        self._type_url = type_url
        self._payload = message.SerializeToString()
        return self

    def xǁCommandBuilderǁwith_command__mutmut_1(
        self, type_url: str, message: Message
    ) -> "CommandBuilder":
        """Set the command type URL and message."""
        self._type_url = None
        self._payload = message.SerializeToString()
        return self

    def xǁCommandBuilderǁwith_command__mutmut_2(
        self, type_url: str, message: Message
    ) -> "CommandBuilder":
        """Set the command type URL and message."""
        self._type_url = type_url
        self._payload = None
        return self

    xǁCommandBuilderǁwith_command__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁCommandBuilderǁwith_command__mutmut_1": xǁCommandBuilderǁwith_command__mutmut_1,
        "xǁCommandBuilderǁwith_command__mutmut_2": xǁCommandBuilderǁwith_command__mutmut_2,
    }

    def with_command(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁCommandBuilderǁwith_command__mutmut_orig"),
            object.__getattribute__(
                self, "xǁCommandBuilderǁwith_command__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    with_command.__signature__ = _mutmut_signature(
        xǁCommandBuilderǁwith_command__mutmut_orig
    )
    xǁCommandBuilderǁwith_command__mutmut_orig.__name__ = (
        "xǁCommandBuilderǁwith_command"
    )

    def xǁCommandBuilderǁbuild__mutmut_orig(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_1(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_2(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError(None)
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_3(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("XXcommand type_url not setXX")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_4(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("COMMAND TYPE_URL NOT SET")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_5(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is not None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_6(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError(None)

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_7(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("XXcommand payload not setXX")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_8(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("COMMAND PAYLOAD NOT SET")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_9(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = None

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_10(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id and str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_11(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(None)

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_12(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = None
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_13(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=None,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_14(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=None,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_15(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_16(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_17(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(None)

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_18(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(None))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_19(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = None
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_20(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=None, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_21(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=None)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_22(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_23(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(
            type_url=self._type_url,
        )
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_24(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = None
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_25(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=None)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_26(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(None)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_27(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = None
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_28(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(None)
        book.pages.append(page)
        return book

    def xǁCommandBuilderǁbuild__mutmut_29(self) -> CommandBook:
        """Build the CommandBook without executing."""
        if self._err:
            raise self._err
        if not self._type_url:
            raise InvalidArgumentError("command type_url not set")
        if self._payload is None:
            raise InvalidArgumentError("command payload not set")

        correlation_id = self._correlation_id or str(uuid4())

        cover = Cover(
            domain=self._domain,
            correlation_id=correlation_id,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))

        command_any = ProtoAny(type_url=self._type_url, value=self._payload)
        page = CommandPage(sequence=self._sequence)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(None)
        return book

    xǁCommandBuilderǁbuild__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁCommandBuilderǁbuild__mutmut_1": xǁCommandBuilderǁbuild__mutmut_1,
        "xǁCommandBuilderǁbuild__mutmut_2": xǁCommandBuilderǁbuild__mutmut_2,
        "xǁCommandBuilderǁbuild__mutmut_3": xǁCommandBuilderǁbuild__mutmut_3,
        "xǁCommandBuilderǁbuild__mutmut_4": xǁCommandBuilderǁbuild__mutmut_4,
        "xǁCommandBuilderǁbuild__mutmut_5": xǁCommandBuilderǁbuild__mutmut_5,
        "xǁCommandBuilderǁbuild__mutmut_6": xǁCommandBuilderǁbuild__mutmut_6,
        "xǁCommandBuilderǁbuild__mutmut_7": xǁCommandBuilderǁbuild__mutmut_7,
        "xǁCommandBuilderǁbuild__mutmut_8": xǁCommandBuilderǁbuild__mutmut_8,
        "xǁCommandBuilderǁbuild__mutmut_9": xǁCommandBuilderǁbuild__mutmut_9,
        "xǁCommandBuilderǁbuild__mutmut_10": xǁCommandBuilderǁbuild__mutmut_10,
        "xǁCommandBuilderǁbuild__mutmut_11": xǁCommandBuilderǁbuild__mutmut_11,
        "xǁCommandBuilderǁbuild__mutmut_12": xǁCommandBuilderǁbuild__mutmut_12,
        "xǁCommandBuilderǁbuild__mutmut_13": xǁCommandBuilderǁbuild__mutmut_13,
        "xǁCommandBuilderǁbuild__mutmut_14": xǁCommandBuilderǁbuild__mutmut_14,
        "xǁCommandBuilderǁbuild__mutmut_15": xǁCommandBuilderǁbuild__mutmut_15,
        "xǁCommandBuilderǁbuild__mutmut_16": xǁCommandBuilderǁbuild__mutmut_16,
        "xǁCommandBuilderǁbuild__mutmut_17": xǁCommandBuilderǁbuild__mutmut_17,
        "xǁCommandBuilderǁbuild__mutmut_18": xǁCommandBuilderǁbuild__mutmut_18,
        "xǁCommandBuilderǁbuild__mutmut_19": xǁCommandBuilderǁbuild__mutmut_19,
        "xǁCommandBuilderǁbuild__mutmut_20": xǁCommandBuilderǁbuild__mutmut_20,
        "xǁCommandBuilderǁbuild__mutmut_21": xǁCommandBuilderǁbuild__mutmut_21,
        "xǁCommandBuilderǁbuild__mutmut_22": xǁCommandBuilderǁbuild__mutmut_22,
        "xǁCommandBuilderǁbuild__mutmut_23": xǁCommandBuilderǁbuild__mutmut_23,
        "xǁCommandBuilderǁbuild__mutmut_24": xǁCommandBuilderǁbuild__mutmut_24,
        "xǁCommandBuilderǁbuild__mutmut_25": xǁCommandBuilderǁbuild__mutmut_25,
        "xǁCommandBuilderǁbuild__mutmut_26": xǁCommandBuilderǁbuild__mutmut_26,
        "xǁCommandBuilderǁbuild__mutmut_27": xǁCommandBuilderǁbuild__mutmut_27,
        "xǁCommandBuilderǁbuild__mutmut_28": xǁCommandBuilderǁbuild__mutmut_28,
        "xǁCommandBuilderǁbuild__mutmut_29": xǁCommandBuilderǁbuild__mutmut_29,
    }

    def build(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁCommandBuilderǁbuild__mutmut_orig"),
            object.__getattribute__(self, "xǁCommandBuilderǁbuild__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    build.__signature__ = _mutmut_signature(xǁCommandBuilderǁbuild__mutmut_orig)
    xǁCommandBuilderǁbuild__mutmut_orig.__name__ = "xǁCommandBuilderǁbuild"

    def xǁCommandBuilderǁexecute__mutmut_orig(self) -> CommandResponse:
        """Build and execute the command."""
        cmd = self.build()
        return self._client.handle(cmd)

    def xǁCommandBuilderǁexecute__mutmut_1(self) -> CommandResponse:
        """Build and execute the command."""
        cmd = None
        return self._client.handle(cmd)

    def xǁCommandBuilderǁexecute__mutmut_2(self) -> CommandResponse:
        """Build and execute the command."""
        cmd = self.build()
        return self._client.handle(None)

    xǁCommandBuilderǁexecute__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁCommandBuilderǁexecute__mutmut_1": xǁCommandBuilderǁexecute__mutmut_1,
        "xǁCommandBuilderǁexecute__mutmut_2": xǁCommandBuilderǁexecute__mutmut_2,
    }

    def execute(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁCommandBuilderǁexecute__mutmut_orig"),
            object.__getattribute__(self, "xǁCommandBuilderǁexecute__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    execute.__signature__ = _mutmut_signature(xǁCommandBuilderǁexecute__mutmut_orig)
    xǁCommandBuilderǁexecute__mutmut_orig.__name__ = "xǁCommandBuilderǁexecute"


class QueryBuilder:
    """Fluent builder for constructing and executing queries."""

    def xǁQueryBuilderǁ__init____mutmut_orig(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_1(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = None
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_2(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = None
        self._root = root
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_3(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = None
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_4(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = ""
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_5(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = ""
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_6(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = ""
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_7(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = ""
        self._err: Optional[Exception] = None

    def xǁQueryBuilderǁ__init____mutmut_8(
        self,
        client: QueryClient,
        domain: str,
        root: Optional[PyUUID] = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: Optional[str] = None
        self._range: Optional[SequenceRange] = None
        self._temporal: Optional[TemporalQuery] = None
        self._edition: Optional[str] = None
        self._err: Optional[Exception] = ""

    xǁQueryBuilderǁ__init____mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁ__init____mutmut_1": xǁQueryBuilderǁ__init____mutmut_1,
        "xǁQueryBuilderǁ__init____mutmut_2": xǁQueryBuilderǁ__init____mutmut_2,
        "xǁQueryBuilderǁ__init____mutmut_3": xǁQueryBuilderǁ__init____mutmut_3,
        "xǁQueryBuilderǁ__init____mutmut_4": xǁQueryBuilderǁ__init____mutmut_4,
        "xǁQueryBuilderǁ__init____mutmut_5": xǁQueryBuilderǁ__init____mutmut_5,
        "xǁQueryBuilderǁ__init____mutmut_6": xǁQueryBuilderǁ__init____mutmut_6,
        "xǁQueryBuilderǁ__init____mutmut_7": xǁQueryBuilderǁ__init____mutmut_7,
        "xǁQueryBuilderǁ__init____mutmut_8": xǁQueryBuilderǁ__init____mutmut_8,
    }

    def __init__(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁ__init____mutmut_orig"),
            object.__getattribute__(self, "xǁQueryBuilderǁ__init____mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    __init__.__signature__ = _mutmut_signature(xǁQueryBuilderǁ__init____mutmut_orig)
    xǁQueryBuilderǁ__init____mutmut_orig.__name__ = "xǁQueryBuilderǁ__init__"

    def xǁQueryBuilderǁby_correlation_id__mutmut_orig(self, id: str) -> "QueryBuilder":
        """Query by correlation ID instead of root."""
        self._correlation_id = id
        self._root = None
        return self

    def xǁQueryBuilderǁby_correlation_id__mutmut_1(self, id: str) -> "QueryBuilder":
        """Query by correlation ID instead of root."""
        self._correlation_id = None
        self._root = None
        return self

    def xǁQueryBuilderǁby_correlation_id__mutmut_2(self, id: str) -> "QueryBuilder":
        """Query by correlation ID instead of root."""
        self._correlation_id = id
        self._root = ""
        return self

    xǁQueryBuilderǁby_correlation_id__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁby_correlation_id__mutmut_1": xǁQueryBuilderǁby_correlation_id__mutmut_1,
        "xǁQueryBuilderǁby_correlation_id__mutmut_2": xǁQueryBuilderǁby_correlation_id__mutmut_2,
    }

    def by_correlation_id(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(
                self, "xǁQueryBuilderǁby_correlation_id__mutmut_orig"
            ),
            object.__getattribute__(
                self, "xǁQueryBuilderǁby_correlation_id__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    by_correlation_id.__signature__ = _mutmut_signature(
        xǁQueryBuilderǁby_correlation_id__mutmut_orig
    )
    xǁQueryBuilderǁby_correlation_id__mutmut_orig.__name__ = (
        "xǁQueryBuilderǁby_correlation_id"
    )

    def xǁQueryBuilderǁwith_edition__mutmut_orig(self, edition: str) -> "QueryBuilder":
        """Query events from a specific edition."""
        self._edition = edition
        return self

    def xǁQueryBuilderǁwith_edition__mutmut_1(self, edition: str) -> "QueryBuilder":
        """Query events from a specific edition."""
        self._edition = None
        return self

    xǁQueryBuilderǁwith_edition__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁwith_edition__mutmut_1": xǁQueryBuilderǁwith_edition__mutmut_1
    }

    def with_edition(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁwith_edition__mutmut_orig"),
            object.__getattribute__(
                self, "xǁQueryBuilderǁwith_edition__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    with_edition.__signature__ = _mutmut_signature(
        xǁQueryBuilderǁwith_edition__mutmut_orig
    )
    xǁQueryBuilderǁwith_edition__mutmut_orig.__name__ = "xǁQueryBuilderǁwith_edition"

    def xǁQueryBuilderǁrange__mutmut_orig(self, lower: int) -> "QueryBuilder":
        """Query a range of sequences from lower (inclusive)."""
        self._range = SequenceRange(lower=lower)
        return self

    def xǁQueryBuilderǁrange__mutmut_1(self, lower: int) -> "QueryBuilder":
        """Query a range of sequences from lower (inclusive)."""
        self._range = None
        return self

    def xǁQueryBuilderǁrange__mutmut_2(self, lower: int) -> "QueryBuilder":
        """Query a range of sequences from lower (inclusive)."""
        self._range = SequenceRange(lower=None)
        return self

    xǁQueryBuilderǁrange__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁrange__mutmut_1": xǁQueryBuilderǁrange__mutmut_1,
        "xǁQueryBuilderǁrange__mutmut_2": xǁQueryBuilderǁrange__mutmut_2,
    }

    def range(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁrange__mutmut_orig"),
            object.__getattribute__(self, "xǁQueryBuilderǁrange__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    range.__signature__ = _mutmut_signature(xǁQueryBuilderǁrange__mutmut_orig)
    xǁQueryBuilderǁrange__mutmut_orig.__name__ = "xǁQueryBuilderǁrange"

    def xǁQueryBuilderǁrange_to__mutmut_orig(
        self, lower: int, upper: int
    ) -> "QueryBuilder":
        """Query a range of sequences with upper bound (inclusive)."""
        self._range = SequenceRange(lower=lower, upper=upper)
        return self

    def xǁQueryBuilderǁrange_to__mutmut_1(
        self, lower: int, upper: int
    ) -> "QueryBuilder":
        """Query a range of sequences with upper bound (inclusive)."""
        self._range = None
        return self

    def xǁQueryBuilderǁrange_to__mutmut_2(
        self, lower: int, upper: int
    ) -> "QueryBuilder":
        """Query a range of sequences with upper bound (inclusive)."""
        self._range = SequenceRange(lower=None, upper=upper)
        return self

    def xǁQueryBuilderǁrange_to__mutmut_3(
        self, lower: int, upper: int
    ) -> "QueryBuilder":
        """Query a range of sequences with upper bound (inclusive)."""
        self._range = SequenceRange(lower=lower, upper=None)
        return self

    def xǁQueryBuilderǁrange_to__mutmut_4(
        self, lower: int, upper: int
    ) -> "QueryBuilder":
        """Query a range of sequences with upper bound (inclusive)."""
        self._range = SequenceRange(upper=upper)
        return self

    def xǁQueryBuilderǁrange_to__mutmut_5(
        self, lower: int, upper: int
    ) -> "QueryBuilder":
        """Query a range of sequences with upper bound (inclusive)."""
        self._range = SequenceRange(
            lower=lower,
        )
        return self

    xǁQueryBuilderǁrange_to__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁrange_to__mutmut_1": xǁQueryBuilderǁrange_to__mutmut_1,
        "xǁQueryBuilderǁrange_to__mutmut_2": xǁQueryBuilderǁrange_to__mutmut_2,
        "xǁQueryBuilderǁrange_to__mutmut_3": xǁQueryBuilderǁrange_to__mutmut_3,
        "xǁQueryBuilderǁrange_to__mutmut_4": xǁQueryBuilderǁrange_to__mutmut_4,
        "xǁQueryBuilderǁrange_to__mutmut_5": xǁQueryBuilderǁrange_to__mutmut_5,
    }

    def range_to(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁrange_to__mutmut_orig"),
            object.__getattribute__(self, "xǁQueryBuilderǁrange_to__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    range_to.__signature__ = _mutmut_signature(xǁQueryBuilderǁrange_to__mutmut_orig)
    xǁQueryBuilderǁrange_to__mutmut_orig.__name__ = "xǁQueryBuilderǁrange_to"

    def xǁQueryBuilderǁas_of_sequence__mutmut_orig(self, seq: int) -> "QueryBuilder":
        """Query state as of a specific sequence number."""
        self._temporal = TemporalQuery(as_of_sequence=seq)
        return self

    def xǁQueryBuilderǁas_of_sequence__mutmut_1(self, seq: int) -> "QueryBuilder":
        """Query state as of a specific sequence number."""
        self._temporal = None
        return self

    def xǁQueryBuilderǁas_of_sequence__mutmut_2(self, seq: int) -> "QueryBuilder":
        """Query state as of a specific sequence number."""
        self._temporal = TemporalQuery(as_of_sequence=None)
        return self

    xǁQueryBuilderǁas_of_sequence__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁas_of_sequence__mutmut_1": xǁQueryBuilderǁas_of_sequence__mutmut_1,
        "xǁQueryBuilderǁas_of_sequence__mutmut_2": xǁQueryBuilderǁas_of_sequence__mutmut_2,
    }

    def as_of_sequence(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁas_of_sequence__mutmut_orig"),
            object.__getattribute__(
                self, "xǁQueryBuilderǁas_of_sequence__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    as_of_sequence.__signature__ = _mutmut_signature(
        xǁQueryBuilderǁas_of_sequence__mutmut_orig
    )
    xǁQueryBuilderǁas_of_sequence__mutmut_orig.__name__ = (
        "xǁQueryBuilderǁas_of_sequence"
    )

    def xǁQueryBuilderǁas_of_time__mutmut_orig(self, rfc3339: str) -> "QueryBuilder":
        """Query state as of a specific timestamp (RFC3339 format)."""
        try:
            ts = parse_timestamp(rfc3339)
            self._temporal = TemporalQuery()
            self._temporal.as_of_time.CopyFrom(ts)
        except Exception as e:
            self._err = e
        return self

    def xǁQueryBuilderǁas_of_time__mutmut_1(self, rfc3339: str) -> "QueryBuilder":
        """Query state as of a specific timestamp (RFC3339 format)."""
        try:
            ts = None
            self._temporal = TemporalQuery()
            self._temporal.as_of_time.CopyFrom(ts)
        except Exception as e:
            self._err = e
        return self

    def xǁQueryBuilderǁas_of_time__mutmut_2(self, rfc3339: str) -> "QueryBuilder":
        """Query state as of a specific timestamp (RFC3339 format)."""
        try:
            ts = parse_timestamp(None)
            self._temporal = TemporalQuery()
            self._temporal.as_of_time.CopyFrom(ts)
        except Exception as e:
            self._err = e
        return self

    def xǁQueryBuilderǁas_of_time__mutmut_3(self, rfc3339: str) -> "QueryBuilder":
        """Query state as of a specific timestamp (RFC3339 format)."""
        try:
            ts = parse_timestamp(rfc3339)
            self._temporal = None
            self._temporal.as_of_time.CopyFrom(ts)
        except Exception as e:
            self._err = e
        return self

    def xǁQueryBuilderǁas_of_time__mutmut_4(self, rfc3339: str) -> "QueryBuilder":
        """Query state as of a specific timestamp (RFC3339 format)."""
        try:
            ts = parse_timestamp(rfc3339)
            self._temporal = TemporalQuery()
            self._temporal.as_of_time.CopyFrom(None)
        except Exception as e:
            self._err = e
        return self

    def xǁQueryBuilderǁas_of_time__mutmut_5(self, rfc3339: str) -> "QueryBuilder":
        """Query state as of a specific timestamp (RFC3339 format)."""
        try:
            ts = parse_timestamp(rfc3339)
            self._temporal = TemporalQuery()
            self._temporal.as_of_time.CopyFrom(ts)
        except Exception as e:
            self._err = None
        return self

    xǁQueryBuilderǁas_of_time__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁas_of_time__mutmut_1": xǁQueryBuilderǁas_of_time__mutmut_1,
        "xǁQueryBuilderǁas_of_time__mutmut_2": xǁQueryBuilderǁas_of_time__mutmut_2,
        "xǁQueryBuilderǁas_of_time__mutmut_3": xǁQueryBuilderǁas_of_time__mutmut_3,
        "xǁQueryBuilderǁas_of_time__mutmut_4": xǁQueryBuilderǁas_of_time__mutmut_4,
        "xǁQueryBuilderǁas_of_time__mutmut_5": xǁQueryBuilderǁas_of_time__mutmut_5,
    }

    def as_of_time(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁas_of_time__mutmut_orig"),
            object.__getattribute__(self, "xǁQueryBuilderǁas_of_time__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    as_of_time.__signature__ = _mutmut_signature(xǁQueryBuilderǁas_of_time__mutmut_orig)
    xǁQueryBuilderǁas_of_time__mutmut_orig.__name__ = "xǁQueryBuilderǁas_of_time"

    def xǁQueryBuilderǁbuild__mutmut_orig(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_1(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = None
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_2(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=None,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_3(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=None,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_4(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_5(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_6(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id and "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_7(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "XXXX",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_8(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(None)
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_9(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(None))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_10(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(None)

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_11(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(None))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_12(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = None
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_13(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(None)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_14(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(None)
        elif self._temporal:
            query.temporal.CopyFrom(self._temporal)

        return query

    def xǁQueryBuilderǁbuild__mutmut_15(self) -> Query:
        """Build the Query without executing."""
        if self._err:
            raise self._err

        cover = Cover(
            domain=self._domain,
            correlation_id=self._correlation_id or "",
        )
        if self._root:
            cover.root.CopyFrom(uuid_to_proto(self._root))
        if self._edition:
            cover.edition.CopyFrom(implicit_edition(self._edition))

        query = Query()
        query.cover.CopyFrom(cover)

        if self._range:
            query.range.CopyFrom(self._range)
        elif self._temporal:
            query.temporal.CopyFrom(None)

        return query

    xǁQueryBuilderǁbuild__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁbuild__mutmut_1": xǁQueryBuilderǁbuild__mutmut_1,
        "xǁQueryBuilderǁbuild__mutmut_2": xǁQueryBuilderǁbuild__mutmut_2,
        "xǁQueryBuilderǁbuild__mutmut_3": xǁQueryBuilderǁbuild__mutmut_3,
        "xǁQueryBuilderǁbuild__mutmut_4": xǁQueryBuilderǁbuild__mutmut_4,
        "xǁQueryBuilderǁbuild__mutmut_5": xǁQueryBuilderǁbuild__mutmut_5,
        "xǁQueryBuilderǁbuild__mutmut_6": xǁQueryBuilderǁbuild__mutmut_6,
        "xǁQueryBuilderǁbuild__mutmut_7": xǁQueryBuilderǁbuild__mutmut_7,
        "xǁQueryBuilderǁbuild__mutmut_8": xǁQueryBuilderǁbuild__mutmut_8,
        "xǁQueryBuilderǁbuild__mutmut_9": xǁQueryBuilderǁbuild__mutmut_9,
        "xǁQueryBuilderǁbuild__mutmut_10": xǁQueryBuilderǁbuild__mutmut_10,
        "xǁQueryBuilderǁbuild__mutmut_11": xǁQueryBuilderǁbuild__mutmut_11,
        "xǁQueryBuilderǁbuild__mutmut_12": xǁQueryBuilderǁbuild__mutmut_12,
        "xǁQueryBuilderǁbuild__mutmut_13": xǁQueryBuilderǁbuild__mutmut_13,
        "xǁQueryBuilderǁbuild__mutmut_14": xǁQueryBuilderǁbuild__mutmut_14,
        "xǁQueryBuilderǁbuild__mutmut_15": xǁQueryBuilderǁbuild__mutmut_15,
    }

    def build(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁbuild__mutmut_orig"),
            object.__getattribute__(self, "xǁQueryBuilderǁbuild__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    build.__signature__ = _mutmut_signature(xǁQueryBuilderǁbuild__mutmut_orig)
    xǁQueryBuilderǁbuild__mutmut_orig.__name__ = "xǁQueryBuilderǁbuild"

    def xǁQueryBuilderǁget_event_book__mutmut_orig(self) -> EventBook:
        """Execute the query and return a single EventBook."""
        query = self.build()
        return self._client.get_event_book(query)

    def xǁQueryBuilderǁget_event_book__mutmut_1(self) -> EventBook:
        """Execute the query and return a single EventBook."""
        query = None
        return self._client.get_event_book(query)

    def xǁQueryBuilderǁget_event_book__mutmut_2(self) -> EventBook:
        """Execute the query and return a single EventBook."""
        query = self.build()
        return self._client.get_event_book(None)

    xǁQueryBuilderǁget_event_book__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁget_event_book__mutmut_1": xǁQueryBuilderǁget_event_book__mutmut_1,
        "xǁQueryBuilderǁget_event_book__mutmut_2": xǁQueryBuilderǁget_event_book__mutmut_2,
    }

    def get_event_book(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁget_event_book__mutmut_orig"),
            object.__getattribute__(
                self, "xǁQueryBuilderǁget_event_book__mutmut_mutants"
            ),
            args,
            kwargs,
            self,
        )
        return result

    get_event_book.__signature__ = _mutmut_signature(
        xǁQueryBuilderǁget_event_book__mutmut_orig
    )
    xǁQueryBuilderǁget_event_book__mutmut_orig.__name__ = (
        "xǁQueryBuilderǁget_event_book"
    )

    def xǁQueryBuilderǁget_events__mutmut_orig(self) -> list[EventBook]:
        """Execute the query and return all matching EventBooks."""
        query = self.build()
        return self._client.get_events(query)

    def xǁQueryBuilderǁget_events__mutmut_1(self) -> list[EventBook]:
        """Execute the query and return all matching EventBooks."""
        query = None
        return self._client.get_events(query)

    def xǁQueryBuilderǁget_events__mutmut_2(self) -> list[EventBook]:
        """Execute the query and return all matching EventBooks."""
        query = self.build()
        return self._client.get_events(None)

    xǁQueryBuilderǁget_events__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁget_events__mutmut_1": xǁQueryBuilderǁget_events__mutmut_1,
        "xǁQueryBuilderǁget_events__mutmut_2": xǁQueryBuilderǁget_events__mutmut_2,
    }

    def get_events(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁget_events__mutmut_orig"),
            object.__getattribute__(self, "xǁQueryBuilderǁget_events__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    get_events.__signature__ = _mutmut_signature(xǁQueryBuilderǁget_events__mutmut_orig)
    xǁQueryBuilderǁget_events__mutmut_orig.__name__ = "xǁQueryBuilderǁget_events"

    def xǁQueryBuilderǁget_pages__mutmut_orig(self) -> list[EventPage]:
        """Execute the query and return just the event pages."""
        book = self.get_event_book()
        return list(book.pages)

    def xǁQueryBuilderǁget_pages__mutmut_1(self) -> list[EventPage]:
        """Execute the query and return just the event pages."""
        book = None
        return list(book.pages)

    def xǁQueryBuilderǁget_pages__mutmut_2(self) -> list[EventPage]:
        """Execute the query and return just the event pages."""
        book = self.get_event_book()
        return list(None)

    xǁQueryBuilderǁget_pages__mutmut_mutants: ClassVar[MutantDict] = {
        "xǁQueryBuilderǁget_pages__mutmut_1": xǁQueryBuilderǁget_pages__mutmut_1,
        "xǁQueryBuilderǁget_pages__mutmut_2": xǁQueryBuilderǁget_pages__mutmut_2,
    }

    def get_pages(self, *args, **kwargs):
        result = _mutmut_trampoline(
            object.__getattribute__(self, "xǁQueryBuilderǁget_pages__mutmut_orig"),
            object.__getattribute__(self, "xǁQueryBuilderǁget_pages__mutmut_mutants"),
            args,
            kwargs,
            self,
        )
        return result

    get_pages.__signature__ = _mutmut_signature(xǁQueryBuilderǁget_pages__mutmut_orig)
    xǁQueryBuilderǁget_pages__mutmut_orig.__name__ = "xǁQueryBuilderǁget_pages"


# Convenience functions for creating builders


def x_command__mutmut_orig(
    client: AggregateClient, domain: str, root: PyUUID
) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(client, domain, root)


# Convenience functions for creating builders


def x_command__mutmut_1(
    client: AggregateClient, domain: str, root: PyUUID
) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(None, domain, root)


# Convenience functions for creating builders


def x_command__mutmut_2(
    client: AggregateClient, domain: str, root: PyUUID
) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(client, None, root)


# Convenience functions for creating builders


def x_command__mutmut_3(
    client: AggregateClient, domain: str, root: PyUUID
) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(client, domain, None)


# Convenience functions for creating builders


def x_command__mutmut_4(
    client: AggregateClient, domain: str, root: PyUUID
) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(domain, root)


# Convenience functions for creating builders


def x_command__mutmut_5(
    client: AggregateClient, domain: str, root: PyUUID
) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(client, root)


# Convenience functions for creating builders


def x_command__mutmut_6(
    client: AggregateClient, domain: str, root: PyUUID
) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(
        client,
        domain,
    )


x_command__mutmut_mutants: ClassVar[MutantDict] = {
    "x_command__mutmut_1": x_command__mutmut_1,
    "x_command__mutmut_2": x_command__mutmut_2,
    "x_command__mutmut_3": x_command__mutmut_3,
    "x_command__mutmut_4": x_command__mutmut_4,
    "x_command__mutmut_5": x_command__mutmut_5,
    "x_command__mutmut_6": x_command__mutmut_6,
}


def command(*args, **kwargs):
    result = _mutmut_trampoline(
        x_command__mutmut_orig, x_command__mutmut_mutants, args, kwargs
    )
    return result


command.__signature__ = _mutmut_signature(x_command__mutmut_orig)
x_command__mutmut_orig.__name__ = "x_command"


def x_command_new__mutmut_orig(client: AggregateClient, domain: str) -> CommandBuilder:
    """Start building a command for a new aggregate."""
    return CommandBuilder(client, domain)


def x_command_new__mutmut_1(client: AggregateClient, domain: str) -> CommandBuilder:
    """Start building a command for a new aggregate."""
    return CommandBuilder(None, domain)


def x_command_new__mutmut_2(client: AggregateClient, domain: str) -> CommandBuilder:
    """Start building a command for a new aggregate."""
    return CommandBuilder(client, None)


def x_command_new__mutmut_3(client: AggregateClient, domain: str) -> CommandBuilder:
    """Start building a command for a new aggregate."""
    return CommandBuilder(domain)


def x_command_new__mutmut_4(client: AggregateClient, domain: str) -> CommandBuilder:
    """Start building a command for a new aggregate."""
    return CommandBuilder(
        client,
    )


x_command_new__mutmut_mutants: ClassVar[MutantDict] = {
    "x_command_new__mutmut_1": x_command_new__mutmut_1,
    "x_command_new__mutmut_2": x_command_new__mutmut_2,
    "x_command_new__mutmut_3": x_command_new__mutmut_3,
    "x_command_new__mutmut_4": x_command_new__mutmut_4,
}


def command_new(*args, **kwargs):
    result = _mutmut_trampoline(
        x_command_new__mutmut_orig, x_command_new__mutmut_mutants, args, kwargs
    )
    return result


command_new.__signature__ = _mutmut_signature(x_command_new__mutmut_orig)
x_command_new__mutmut_orig.__name__ = "x_command_new"


def x_query__mutmut_orig(
    client: QueryClient, domain: str, root: PyUUID
) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(client, domain, root)


def x_query__mutmut_1(client: QueryClient, domain: str, root: PyUUID) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(None, domain, root)


def x_query__mutmut_2(client: QueryClient, domain: str, root: PyUUID) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(client, None, root)


def x_query__mutmut_3(client: QueryClient, domain: str, root: PyUUID) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(client, domain, None)


def x_query__mutmut_4(client: QueryClient, domain: str, root: PyUUID) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(domain, root)


def x_query__mutmut_5(client: QueryClient, domain: str, root: PyUUID) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(client, root)


def x_query__mutmut_6(client: QueryClient, domain: str, root: PyUUID) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(
        client,
        domain,
    )


x_query__mutmut_mutants: ClassVar[MutantDict] = {
    "x_query__mutmut_1": x_query__mutmut_1,
    "x_query__mutmut_2": x_query__mutmut_2,
    "x_query__mutmut_3": x_query__mutmut_3,
    "x_query__mutmut_4": x_query__mutmut_4,
    "x_query__mutmut_5": x_query__mutmut_5,
    "x_query__mutmut_6": x_query__mutmut_6,
}


def query(*args, **kwargs):
    result = _mutmut_trampoline(
        x_query__mutmut_orig, x_query__mutmut_mutants, args, kwargs
    )
    return result


query.__signature__ = _mutmut_signature(x_query__mutmut_orig)
x_query__mutmut_orig.__name__ = "x_query"


def x_query_domain__mutmut_orig(client: QueryClient, domain: str) -> QueryBuilder:
    """Start building a query by domain only (use with by_correlation_id)."""
    return QueryBuilder(client, domain)


def x_query_domain__mutmut_1(client: QueryClient, domain: str) -> QueryBuilder:
    """Start building a query by domain only (use with by_correlation_id)."""
    return QueryBuilder(None, domain)


def x_query_domain__mutmut_2(client: QueryClient, domain: str) -> QueryBuilder:
    """Start building a query by domain only (use with by_correlation_id)."""
    return QueryBuilder(client, None)


def x_query_domain__mutmut_3(client: QueryClient, domain: str) -> QueryBuilder:
    """Start building a query by domain only (use with by_correlation_id)."""
    return QueryBuilder(domain)


def x_query_domain__mutmut_4(client: QueryClient, domain: str) -> QueryBuilder:
    """Start building a query by domain only (use with by_correlation_id)."""
    return QueryBuilder(
        client,
    )


x_query_domain__mutmut_mutants: ClassVar[MutantDict] = {
    "x_query_domain__mutmut_1": x_query_domain__mutmut_1,
    "x_query_domain__mutmut_2": x_query_domain__mutmut_2,
    "x_query_domain__mutmut_3": x_query_domain__mutmut_3,
    "x_query_domain__mutmut_4": x_query_domain__mutmut_4,
}


def query_domain(*args, **kwargs):
    result = _mutmut_trampoline(
        x_query_domain__mutmut_orig, x_query_domain__mutmut_mutants, args, kwargs
    )
    return result


query_domain.__signature__ = _mutmut_signature(x_query_domain__mutmut_orig)
x_query_domain__mutmut_orig.__name__ = "x_query_domain"
