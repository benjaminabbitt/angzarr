"""Fluent builders for commands and queries."""

from uuid import UUID as PyUUID
from uuid import uuid4

from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.message import Message

from .client import CommandHandlerClient, QueryClient
from .errors import InvalidArgumentError
from .helpers import implicit_edition, parse_timestamp, uuid_to_proto
from .proto.angzarr import (
    CommandBook,
    CommandPage,
    CommandRequest,
    CommandResponse,
    Cover,
    EventBook,
    EventPage,
    PageHeader,
    Query,
    SequenceRange,
    SyncMode,
    TemporalQuery,
)


class CommandBuilder:
    """Fluent builder for constructing and executing commands."""

    def __init__(
        self,
        client: CommandHandlerClient,
        domain: str,
        root: PyUUID | None = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: str | None = None
        self._sequence: int = 0
        self._type_url: str | None = None
        self._payload: bytes | None = None
        self._err: Exception | None = None

    def with_correlation_id(self, id: str) -> "CommandBuilder":
        """Set the correlation ID for request tracing."""
        self._correlation_id = id
        return self

    def with_sequence(self, seq: int) -> "CommandBuilder":
        """Set the expected sequence number for optimistic locking."""
        self._sequence = seq
        return self

    def with_command(self, type_url: str, message: Message) -> "CommandBuilder":
        """Set the command type URL and message."""
        self._type_url = type_url
        self._payload = message.SerializeToString()
        return self

    def build(self) -> CommandBook:
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
        header = PageHeader(sequence=self._sequence)
        page = CommandPage(header=header)
        page.command.CopyFrom(command_any)

        book = CommandBook()
        book.cover.CopyFrom(cover)
        book.pages.append(page)
        return book

    def execute(
        self, sync_mode: SyncMode = SyncMode.SYNC_MODE_ASYNC
    ) -> CommandResponse:
        """Build and execute the command.

        Args:
            sync_mode: Execution mode (ASYNC, SIMPLE, or CASCADE).
                      Defaults to ASYNC for fire-and-forget behavior.
        """
        cmd = self.build()
        request = CommandRequest(command=cmd, sync_mode=sync_mode)
        return self._client.handle_command(request)


class QueryBuilder:
    """Fluent builder for constructing and executing queries."""

    def __init__(
        self,
        client: QueryClient,
        domain: str,
        root: PyUUID | None = None,
    ):
        self._client = client
        self._domain = domain
        self._root = root
        self._correlation_id: str | None = None
        self._range: SequenceRange | None = None
        self._temporal: TemporalQuery | None = None
        self._edition: str | None = None
        self._err: Exception | None = None

    def by_correlation_id(self, id: str) -> "QueryBuilder":
        """Query by correlation ID instead of root."""
        self._correlation_id = id
        self._root = None
        return self

    def with_edition(self, edition: str) -> "QueryBuilder":
        """Query events from a specific edition."""
        self._edition = edition
        return self

    def range(self, lower: int) -> "QueryBuilder":
        """Query a range of sequences from lower (inclusive)."""
        self._range = SequenceRange(lower=lower)
        return self

    def range_to(self, lower: int, upper: int) -> "QueryBuilder":
        """Query a range of sequences with upper bound (inclusive)."""
        self._range = SequenceRange(lower=lower, upper=upper)
        return self

    def as_of_sequence(self, seq: int) -> "QueryBuilder":
        """Query state as of a specific sequence number."""
        self._temporal = TemporalQuery(as_of_sequence=seq)
        return self

    def as_of_time(self, rfc3339: str) -> "QueryBuilder":
        """Query state as of a specific timestamp (RFC3339 format)."""
        try:
            ts = parse_timestamp(rfc3339)
            self._temporal = TemporalQuery()
            self._temporal.as_of_time.CopyFrom(ts)
        except Exception as e:
            self._err = e
        return self

    def build(self) -> Query:
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

    def get_event_book(self) -> EventBook:
        """Execute the query and return a single EventBook."""
        query = self.build()
        return self._client.get_event_book(query)

    def get_events(self) -> list[EventBook]:
        """Execute the query and return all matching EventBooks."""
        query = self.build()
        return self._client.get_events(query)

    def get_pages(self) -> list[EventPage]:
        """Execute the query and return just the event pages."""
        book = self.get_event_book()
        return list(book.pages)


# Convenience functions for creating builders


def command(client: CommandHandlerClient, domain: str, root: PyUUID) -> CommandBuilder:
    """Start building a command for an existing aggregate."""
    return CommandBuilder(client, domain, root)


def command_new(client: CommandHandlerClient, domain: str) -> CommandBuilder:
    """Start building a command for a new aggregate."""
    return CommandBuilder(client, domain)


def query(client: QueryClient, domain: str, root: PyUUID) -> QueryBuilder:
    """Start building a query for a specific aggregate."""
    return QueryBuilder(client, domain, root)


def query_domain(client: QueryClient, domain: str) -> QueryBuilder:
    """Start building a query by domain only (use with by_correlation_id)."""
    return QueryBuilder(client, domain)
