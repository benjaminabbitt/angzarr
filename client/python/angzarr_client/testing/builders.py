"""Proto message builders for testing.

Provides simplified builders for constructing EventBook, CommandBook,
Cover, and related proto types in tests.
"""

from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

from ..helpers import now as _now
from ..proto.angzarr import (
    UUID,
    CommandBook,
    CommandPage,
    Cover,
    EventBook,
    EventPage,
)


def make_timestamp() -> Timestamp:
    """Create a timestamp for now.

    Alias for angzarr_client.helpers.now() for backwards compatibility.

    Returns:
        Current time as protobuf Timestamp
    """
    return _now()


def pack_event(msg: Message, type_url_prefix: str = "type.googleapis.com/") -> ProtoAny:
    """Pack a protobuf message into Any.

    Args:
        msg: The protobuf message to pack
        type_url_prefix: URL prefix for type identification

    Returns:
        ProtoAny containing the packed message
    """
    event_any = ProtoAny()
    event_any.Pack(msg, type_url_prefix=type_url_prefix)
    return event_any


def make_cover(domain: str, root: bytes, correlation_id: str = "") -> Cover:
    """Create a Cover from domain and root bytes.

    Args:
        domain: The aggregate domain name
        root: The aggregate root as 16 bytes
        correlation_id: Optional correlation ID for cross-domain tracking

    Returns:
        Cover proto with domain and root set
    """
    return Cover(
        domain=domain,
        root=UUID(value=root),
        correlation_id=correlation_id,
    )


def make_event_page(
    sequence: int,
    event: ProtoAny,
) -> EventPage:
    """Create an EventPage.

    Args:
        sequence: The event sequence number
        event: The packed event (ProtoAny)

    Returns:
        EventPage proto
    """
    return EventPage(
        sequence=sequence,
        event=event,
        created_at=_now(),
    )


def make_event_book(
    cover: Cover,
    pages: list[EventPage] = None,
    next_sequence: int = None,
) -> EventBook:
    """Create an EventBook.

    Args:
        cover: The Cover identifying the aggregate
        pages: List of EventPages (defaults to empty)
        next_sequence: Next sequence number (defaults to len(pages))

    Returns:
        EventBook proto
    """
    pages = pages or []
    return EventBook(
        cover=cover,
        pages=pages,
        next_sequence=next_sequence if next_sequence is not None else len(pages),
    )


def make_command_page(sequence: int, command: ProtoAny) -> CommandPage:
    """Create a CommandPage.

    Args:
        sequence: The command sequence number
        command: The packed command (ProtoAny)

    Returns:
        CommandPage proto
    """
    return CommandPage(
        sequence=sequence,
        command=command,
    )


def make_command_book(
    cover: Cover,
    command: ProtoAny,
    sequence: int = 0,
) -> CommandBook:
    """Create a CommandBook with a single command.

    Args:
        cover: The Cover identifying the target aggregate
        command: The packed command (ProtoAny)
        sequence: The command sequence number (defaults to 0)

    Returns:
        CommandBook proto with one page
    """
    return CommandBook(
        cover=cover,
        pages=[make_command_page(sequence, command)],
    )


__all__ = [
    "make_timestamp",
    "pack_event",
    "make_cover",
    "make_event_page",
    "make_event_book",
    "make_command_page",
    "make_command_book",
]
