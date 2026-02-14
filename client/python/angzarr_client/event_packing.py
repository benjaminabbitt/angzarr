"""Event packing utilities for angzarr command handlers.

Wraps protobuf events into EventBook structures with cover, sequence, and timestamp.
"""

from datetime import datetime, timezone
from typing import Sequence

from google.protobuf.any_pb2 import Any
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

from .proto.angzarr import types_pb2 as angzarr
from .helpers import now


def _now_timestamp() -> Timestamp:
    """Internal alias for now()."""
    return now()


def new_event_book(
    command_book: angzarr.CommandBook,
    seq: int,
    event: Any,
) -> angzarr.EventBook:
    """Create an EventBook with a single pre-packed event.

    Args:
        command_book: The command book (cover is extracted from it).
        seq: The sequence number for this event.
        event: The pre-packed Any event.

    Returns:
        An EventBook containing one page with the event.
    """
    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[
            angzarr.EventPage(
                num=seq,
                event=event,
                created_at=_now_timestamp(),
            ),
        ],
    )


def new_event_book_multi(
    command_book: angzarr.CommandBook,
    start_seq: int,
    events: Sequence[Any],
) -> angzarr.EventBook:
    """Create an EventBook with multiple pre-packed events.

    Args:
        command_book: The command book (cover is extracted from it).
        start_seq: The starting sequence number.
        events: List of pre-packed Any events.

    Returns:
        An EventBook containing one page per event with sequential numbering.
    """
    now = _now_timestamp()
    pages = [
        angzarr.EventPage(
            num=start_seq + i,
            event=event,
            created_at=now,
        )
        for i, event in enumerate(events)
    ]
    return angzarr.EventBook(
        cover=command_book.cover,
        pages=pages,
    )


def pack_event(
    cover: angzarr.Cover,
    event: Message,
    seq: int,
    type_url_prefix: str = "type.examples/",
) -> angzarr.EventBook:
    """Pack a single protobuf event into an EventBook.

    Args:
        cover: The event cover (root, metadata).
        event: The protobuf event message.
        seq: The sequence number for this event.
        type_url_prefix: Prefix for the Any type_url.

    Returns:
        An EventBook containing one page with the packed event.
    """
    event_any = Any()
    event_any.Pack(event, type_url_prefix=type_url_prefix)

    return angzarr.EventBook(
        cover=cover,
        pages=[
            angzarr.EventPage(
                num=seq,
                event=event_any,
                created_at=_now_timestamp(),
            ),
        ],
    )


def pack_events(
    cover: angzarr.Cover,
    events: list[Message],
    start_seq: int,
    type_url_prefix: str = "type.examples/",
) -> angzarr.EventBook:
    """Pack multiple protobuf events into an EventBook with sequential numbering.

    Args:
        cover: The event cover (root, metadata).
        events: List of protobuf event messages.
        start_seq: The starting sequence number.
        type_url_prefix: Prefix for the Any type_url.

    Returns:
        An EventBook containing one page per event.
    """
    pages = []
    for i, event in enumerate(events):
        event_any = Any()
        event_any.Pack(event, type_url_prefix=type_url_prefix)
        pages.append(
            angzarr.EventPage(
                num=start_seq + i,
                event=event_any,
                created_at=_now_timestamp(),
            ),
        )

    return angzarr.EventBook(
        cover=cover,
        pages=pages,
    )
