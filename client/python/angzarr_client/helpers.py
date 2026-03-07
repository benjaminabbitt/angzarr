"""Helper functions for working with Angzarr proto types."""

from typing import TypeVar, Union
from uuid import UUID as PyUUID

from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.message import Message
from google.protobuf.timestamp_pb2 import Timestamp

# Type variable for generic message type
T = TypeVar("T", bound=Message)

from .errors import InvalidTimestampError
from .proto.angzarr import (
    UUID,
    CommandBook,
    CommandPage,
    Cover,
    DomainDivergence,
    Edition,
    EventBook,
    EventPage,
    Query,
    SequenceRange,
    TemporalQuery,
)

# Constants matching Rust proto_ext::constants
UNKNOWN_DOMAIN = "unknown"
WILDCARD_DOMAIN = "*"
DEFAULT_EDITION = "angzarr"
META_ANGZARR_DOMAIN = "_angzarr"
PROJECTION_DOMAIN_PREFIX = "projection:"
CORRELATION_ID_HEADER = "x-correlation-id"
TYPE_URL_PREFIX = "type.googleapis.com/"


# Type for Cover-bearing objects
CoverBearer = Union[EventBook, CommandBook, Query, Cover]


def cover_of(obj: CoverBearer) -> Cover | None:
    """Extract the Cover from various proto types."""
    if isinstance(obj, Cover):
        return obj
    if hasattr(obj, "cover"):
        return obj.cover
    return None


def domain(obj: CoverBearer) -> str:
    """Get the domain from a Cover-bearing type, or UNKNOWN_DOMAIN if missing."""
    c = cover_of(obj)
    if c is None or not c.domain:
        return UNKNOWN_DOMAIN
    return c.domain


def correlation_id(obj: CoverBearer) -> str:
    """Get the correlation_id from a Cover-bearing type, or empty string if missing."""
    c = cover_of(obj)
    if c is None:
        return ""
    return c.correlation_id


def has_correlation_id(obj: CoverBearer) -> bool:
    """Return True if the correlation_id is present and non-empty."""
    return bool(correlation_id(obj))


def root_uuid(obj: CoverBearer) -> PyUUID | None:
    """Extract the root UUID from a Cover-bearing type."""
    c = cover_of(obj)
    if c is None or not c.HasField("root"):
        return None
    try:
        return PyUUID(bytes=c.root.value)
    except ValueError:
        return None


def root_id_hex(obj: CoverBearer) -> str:
    """Return the root UUID as a hex string, or empty string if missing."""
    c = cover_of(obj)
    if c is None or not c.HasField("root"):
        return ""
    return c.root.value.hex()


def edition(obj: CoverBearer) -> str:
    """Return the edition name from a Cover-bearing type, defaulting to DEFAULT_EDITION."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") or not c.edition.name:
        return DEFAULT_EDITION
    return c.edition.name


def edition_opt(obj: CoverBearer) -> str | None:
    """Return the edition name as Optional, None if not set."""
    c = cover_of(obj)
    if c is None or not c.HasField("edition") or not c.edition.name:
        return None
    return c.edition.name


def routing_key(obj: CoverBearer) -> str:
    """Compute the bus routing key for a Cover-bearing type."""
    return domain(obj)


def cache_key(obj: CoverBearer) -> str:
    """Generate a cache key based on edition + domain + root."""
    return f"{edition(obj)}:{domain(obj)}:{root_id_hex(obj)}"


# UUID conversion


def uuid_to_proto(u: PyUUID) -> UUID:
    """Convert a Python UUID to a proto UUID."""
    return UUID(value=u.bytes)


def proto_to_uuid(u: UUID) -> PyUUID:
    """Convert a proto UUID to Python UUID."""
    return PyUUID(bytes=u.value)


def bytes_to_uuid_text(b: bytes) -> str:
    """Convert bytes to standard UUID text format.

    If bytes are exactly 16 bytes, formats as UUID (8-4-4-4-12).
    Otherwise returns hex encoding of the bytes.
    """
    if len(b) == 16:
        return str(PyUUID(bytes=b))
    return b.hex()


def proto_uuid_to_text(u: UUID | None) -> str:
    """Convert a proto UUID to text format."""
    if u is None:
        return ""
    return bytes_to_uuid_text(u.value)


def root_id_text(obj: CoverBearer) -> str:
    """Return the root UUID as standard text format (8-4-4-4-12), or empty string if missing."""
    c = cover_of(obj)
    if c is None or not c.HasField("root"):
        return ""
    return bytes_to_uuid_text(c.root.value)


# Edition helpers


def main_timeline() -> Edition:
    """Return an Edition representing the main timeline."""
    return Edition(name=DEFAULT_EDITION)


def implicit_edition(name: str) -> Edition:
    """Create an edition with the given name but no divergences."""
    return Edition(name=name)


def explicit_edition(name: str, divergences: list[DomainDivergence]) -> Edition:
    """Create an edition with divergence points."""
    return Edition(name=name, divergences=divergences)


def is_main_timeline(e: Edition | None) -> bool:
    """Check if an edition represents the main timeline."""
    return e is None or not e.name or e.name == DEFAULT_EDITION


def divergence_for(e: Edition | None, domain_name: str) -> int:
    """Return the divergence sequence for a domain, or -1 if not found."""
    if e is None:
        return -1
    for d in e.divergences:
        if d.domain == domain_name:
            return d.sequence
    return -1


# EventBook helpers


def next_sequence(book: EventBook) -> int:
    """Return the next sequence number from an EventBook.

    The framework computes this value on load.

    Why use book.next_sequence instead of counting events?
    -------------------------------------------------------
    The framework precomputes next_sequence when loading the EventBook because:
    1. **Snapshots**: With snapshots, the EventBook may contain only post-snapshot
       events. Counting events would give the wrong sequence.
    2. **Consistency**: The framework knows the true last sequence from storage.
    3. **Performance**: Avoids iterating through events to find max sequence.

    Command handlers MUST use this value when setting event sequences. Using
    len(book.pages) would produce incorrect sequences when snapshots are involved.
    """
    if book is None:
        return 0
    return book.next_sequence


def event_pages(book: EventBook | None) -> list[EventPage]:
    """Return the event pages from an EventBook, or empty list if None."""
    if book is None:
        return []
    return list(book.pages)


def destination_map(destinations: list[EventBook]) -> dict[str, EventBook]:
    """Build a map from root UUID hex to EventBook for destination lookup.

    Used in multi-destination sagas to look up the correct EventBook
    by aggregate root when setting command sequences.

    Args:
        destinations: List of EventBooks from the saga prepare phase

    Returns:
        Dict mapping root hex string to EventBook. Entries without
        a root are skipped.

    Example:
        dest_map = destination_map(destinations)
        dest_seq = next_sequence(dest_map.get(player_hex))
    """
    result = {}
    for dest in destinations:
        key = root_id_hex(dest)
        if key:
            result[key] = dest
    return result


# CommandBook helpers


def command_pages(book: CommandBook | None) -> list[CommandPage]:
    """Return the command pages from a CommandBook, or empty list if None."""
    if book is None:
        return []
    return list(book.pages)


# CommandResponse helpers


def events_from_response(resp) -> list[EventPage]:
    """Extract the event pages from a CommandResponse."""
    if resp is None or not resp.HasField("events"):
        return []
    return list(resp.events.pages)


# Type URL helpers


def type_url(package_name: str, type_name: str) -> str:
    """Construct a full type URL from a package and type name."""
    return f"{TYPE_URL_PREFIX}{package_name}.{type_name}"


def type_name_from_url(type_url_str: str) -> str:
    """Extract the type name from a type URL."""
    if "." in type_url_str:
        return type_url_str.rsplit(".", 1)[1]
    if "/" in type_url_str:
        return type_url_str.rsplit("/", 1)[1]
    return type_url_str


def type_url_matches(type_url_str: str, type_name: str) -> bool:
    """Check if a type URL matches the given fully qualified type name.

    Args:
        type_url_str: Full type URL (e.g., "type.googleapis.com/examples.CardsDealt")
        type_name: Fully qualified type name (e.g., "examples.CardsDealt")

    Returns:
        True if type_url equals TYPE_URL_PREFIX + type_name
    """
    return type_url_str == TYPE_URL_PREFIX + type_name


# Type-safe reflection helpers


def type_matches(any_proto: ProtoAny, msg_class: type[T]) -> bool:
    """Check if an Any contains a message of the given type using DESCRIPTOR.

    This is preferred over string-based suffix matching.

    Args:
        any_proto: The Any message to check
        msg_class: The protobuf message class to check against

    Returns:
        True if the Any's type_url matches the message class

    Example:
        if type_matches(event_any, PlayerRegistered):
            msg = try_unpack(event_any, PlayerRegistered)
    """
    if any_proto is None:
        return False
    return any_proto.Is(msg_class.DESCRIPTOR)


def try_unpack(any_proto: ProtoAny, msg_class: type[T]) -> T | None:
    """Unpack an Any to msg_class if type matches, returning None otherwise.

    This is type-safe: it only unpacks if the type URL matches exactly.

    Args:
        any_proto: The Any message to unpack
        msg_class: The protobuf message class to unpack into

    Returns:
        The unpacked message if type matches and decoding succeeds, None otherwise

    Example:
        if msg := try_unpack(event_any, PlayerRegistered):
            print(f"Player {msg.player_id} registered")
    """
    if not type_matches(any_proto, msg_class):
        return None
    try:
        msg = msg_class()
        any_proto.Unpack(msg)
        return msg
    except Exception:
        return None


def unpack(any_proto: ProtoAny, msg_class: type[T]) -> T:
    """Unpack an Any to msg_class, raising ValueError if type doesn't match.

    Args:
        any_proto: The Any message to unpack
        msg_class: The protobuf message class to unpack into

    Returns:
        The unpacked message

    Raises:
        ValueError: If type doesn't match or decoding fails
    """
    if not type_matches(any_proto, msg_class):
        expected = full_type_name(msg_class)
        raise ValueError(
            f"type mismatch: expected {expected}, got {any_proto.type_url}"
        )
    msg = msg_class()
    any_proto.Unpack(msg)
    return msg


def full_type_name(msg_class: type[Message]) -> str:
    """Get the fully-qualified type name from a message class DESCRIPTOR.

    Args:
        msg_class: The protobuf message class

    Returns:
        The fully-qualified type name (e.g., "examples.PlayerRegistered")

    Example:
        name = full_type_name(PlayerRegistered)  # "examples.PlayerRegistered"
    """
    return msg_class.DESCRIPTOR.full_name


def full_type_url_for(msg_class: type[Message]) -> str:
    """Get the full type URL for a message class.

    Args:
        msg_class: The protobuf message class

    Returns:
        The full type URL (e.g., "type.googleapis.com/examples.PlayerRegistered")
    """
    return TYPE_URL_PREFIX + full_type_name(msg_class)


# Timestamp helpers


def now() -> Timestamp:
    """Return the current time as a protobuf Timestamp."""
    ts = Timestamp()
    ts.GetCurrentTime()
    return ts


def parse_timestamp(rfc3339: str) -> Timestamp:
    """Parse an RFC3339 timestamp string."""
    try:
        ts = Timestamp()
        ts.FromJsonString(rfc3339)
        return ts
    except ValueError as e:
        raise InvalidTimestampError(str(e)) from e


# Event decoding


def decode_event(page: EventPage, type_suffix: str, msg_class) -> object | None:
    """Attempt to decode an event payload if the type URL matches.

    Args:
        page: The event page to decode
        type_suffix: The expected type URL suffix
        msg_class: The protobuf message class to decode into

    Returns:
        The decoded message if type matches and decoding succeeds, None otherwise
    """
    if page is None or not page.HasField("event"):
        return None
    if not type_url_matches(page.event.type_url, type_suffix):
        return None
    try:
        msg = msg_class()
        page.event.Unpack(msg)
        return msg
    except Exception:
        return None


# Construction helpers


def new_cover(
    domain_name: str,
    root: PyUUID,
    correlation_id_val: str = "",
    edition_val: Edition | None = None,
) -> Cover:
    """Create a new Cover with the given parameters."""
    cover = Cover(
        domain=domain_name,
        root=uuid_to_proto(root),
        correlation_id=correlation_id_val,
    )
    if edition_val is not None:
        cover.edition.CopyFrom(edition_val)
    return cover


def new_command_page(sequence: int, command: ProtoAny) -> CommandPage:
    """Create a command page from a sequence and Any message."""
    page = CommandPage(sequence=sequence)
    page.command.CopyFrom(command)
    return page


def new_command_book(cover: Cover, pages: list[CommandPage]) -> CommandBook:
    """Create a CommandBook with the given cover and pages."""
    book = CommandBook()
    book.cover.CopyFrom(cover)
    book.pages.extend(pages)
    return book


def range_selection(lower: int, upper: int | None = None) -> SequenceRange:
    """Create a sequence range selection."""
    r = SequenceRange(lower=lower)
    if upper is not None:
        r.upper = upper
    return r


def temporal_by_sequence(seq: int) -> TemporalQuery:
    """Create a temporal selection as-of a sequence."""
    return TemporalQuery(as_of_sequence=seq)


def temporal_by_time(ts: Timestamp) -> TemporalQuery:
    """Create a temporal selection as-of a timestamp."""
    tq = TemporalQuery()
    tq.as_of_time.CopyFrom(ts)
    return tq
