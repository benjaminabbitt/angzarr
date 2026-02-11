"""Wrapper classes for Angzarr proto types.

Each wrapper takes a protobuf message in its constructor and provides
extension methods as instance methods.
"""

from typing import Optional, List, Type, TypeVar
from uuid import UUID as PyUUID

from .proto.angzarr import (
    Cover,
    EventBook,
    CommandBook,
    Query,
    EventPage,
    CommandPage,
    CommandResponse,
)
from .helpers import (
    UNKNOWN_DOMAIN,
    DEFAULT_EDITION,
    type_url_matches,
)

T = TypeVar("T")


class CoverW:
    """Wrapper for Cover proto with extension methods."""

    def __init__(self, proto: Cover) -> None:
        self.proto = proto

    def domain(self) -> str:
        """Get the domain, or UNKNOWN_DOMAIN if missing."""
        if not self.proto.domain:
            return UNKNOWN_DOMAIN
        return self.proto.domain

    def correlation_id(self) -> str:
        """Get the correlation_id, or empty string if missing."""
        return self.proto.correlation_id

    def has_correlation_id(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.proto.correlation_id)

    def root_uuid(self) -> Optional[PyUUID]:
        """Extract the root UUID."""
        if not self.proto.HasField("root"):
            return None
        try:
            return PyUUID(bytes=self.proto.root.value)
        except ValueError:
            return None

    def root_id_hex(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        if not self.proto.HasField("root"):
            return ""
        return self.proto.root.value.hex()

    def edition(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        if not self.proto.HasField("edition") or not self.proto.edition.name:
            return DEFAULT_EDITION
        return self.proto.edition.name

    def edition_opt(self) -> Optional[str]:
        """Return the edition name as Optional, None if not set."""
        if not self.proto.HasField("edition") or not self.proto.edition.name:
            return None
        return self.proto.edition.name

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def cache_key(self) -> str:
        """Generate a cache key based on domain + root."""
        return f"{self.domain()}:{self.root_id_hex()}"


class EventBookW:
    """Wrapper for EventBook proto with extension methods."""

    def __init__(self, proto: EventBook) -> None:
        self.proto = proto

    def next_sequence(self) -> int:
        """Return the next sequence number."""
        return self.proto.next_sequence

    def pages(self) -> List["EventPageW"]:
        """Return the event pages as wrapped EventPageW instances."""
        return [EventPageW(p) for p in self.proto.pages]

    def _cover(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def domain(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def correlation_id(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return ""
        return cover.correlation_id

    def has_correlation_id(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.correlation_id())

    def root_uuid(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def root_id_hex(self) -> str:
        """Return the root UUID as a hex string, or empty string if missing."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return ""
        return cover.root.value.hex()

    def edition(self) -> str:
        """Return the edition name, defaulting to DEFAULT_EDITION."""
        cover = self._cover()
        if cover is None or not cover.HasField("edition") or not cover.edition.name:
            return DEFAULT_EDITION
        return cover.edition.name

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def cache_key(self) -> str:
        """Generate a cache key based on domain + root."""
        return f"{self.domain()}:{self.root_id_hex()}"

    def cover_wrapper(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)


class CommandBookW:
    """Wrapper for CommandBook proto with extension methods."""

    def __init__(self, proto: CommandBook) -> None:
        self.proto = proto

    def pages(self) -> List["CommandPageW"]:
        """Return the command pages as wrapped CommandPageW instances."""
        return [CommandPageW(p) for p in self.proto.pages]

    def _cover(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def domain(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def correlation_id(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return ""
        return cover.correlation_id

    def has_correlation_id(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.correlation_id())

    def root_uuid(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def cache_key(self) -> str:
        """Generate a cache key based on domain + root."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return f"{self.domain()}:"
        return f"{self.domain()}:{cover.root.value.hex()}"

    def cover_wrapper(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)


class QueryW:
    """Wrapper for Query proto with extension methods."""

    def __init__(self, proto: Query) -> None:
        self.proto = proto

    def _cover(self) -> Optional[Cover]:
        """Get the cover, or None if not set."""
        if not self.proto.HasField("cover"):
            return None
        return self.proto.cover

    def domain(self) -> str:
        """Get the domain from the cover, or UNKNOWN_DOMAIN if missing."""
        cover = self._cover()
        if cover is None or not cover.domain:
            return UNKNOWN_DOMAIN
        return cover.domain

    def correlation_id(self) -> str:
        """Get the correlation_id from the cover, or empty string if missing."""
        cover = self._cover()
        if cover is None:
            return ""
        return cover.correlation_id

    def has_correlation_id(self) -> bool:
        """Return True if the correlation_id is present and non-empty."""
        return bool(self.correlation_id())

    def root_uuid(self) -> Optional[PyUUID]:
        """Extract the root UUID from the cover."""
        cover = self._cover()
        if cover is None or not cover.HasField("root"):
            return None
        try:
            return PyUUID(bytes=cover.root.value)
        except ValueError:
            return None

    def routing_key(self) -> str:
        """Compute the bus routing key."""
        return self.domain()

    def cover_wrapper(self) -> "CoverW":
        """Return a CoverW wrapping the cover."""
        cover = self._cover()
        if cover is None:
            return CoverW(Cover())
        return CoverW(cover)


class EventPageW:
    """Wrapper for EventPage proto with extension methods."""

    def __init__(self, proto: EventPage) -> None:
        self.proto = proto

    def decode_event(self, type_suffix: str, msg_class: Type[T]) -> Optional[T]:
        """Attempt to decode an event payload if the type URL matches.

        Args:
            type_suffix: The expected type URL suffix
            msg_class: The protobuf message class to decode into

        Returns:
            The decoded message if type matches and decoding succeeds, None otherwise
        """
        if not self.proto.HasField("event"):
            return None
        if not type_url_matches(self.proto.event.type_url, type_suffix):
            return None
        try:
            msg = msg_class()
            self.proto.event.Unpack(msg)
            return msg
        except Exception:
            return None


class CommandPageW:
    """Wrapper for CommandPage proto with extension methods."""

    def __init__(self, proto: CommandPage) -> None:
        self.proto = proto

    def sequence(self) -> int:
        """Return the sequence number."""
        return self.proto.sequence


class CommandResponseW:
    """Wrapper for CommandResponse proto with extension methods."""

    def __init__(self, proto: CommandResponse) -> None:
        self.proto = proto

    def events_book(self) -> Optional["EventBookW"]:
        """Return the events as a wrapped EventBookW, or None if not set."""
        if not self.proto.HasField("events"):
            return None
        return EventBookW(self.proto.events)

    def events(self) -> List["EventPageW"]:
        """Extract the event pages as wrapped EventPageW instances."""
        if not self.proto.HasField("events"):
            return []
        return [EventPageW(p) for p in self.proto.events.pages]
