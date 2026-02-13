"""Tests for builder classes."""

from unittest.mock import Mock, MagicMock
from uuid import UUID as PyUUID, uuid4

import pytest
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.wrappers_pb2 import StringValue

from angzarr_client.proto.angzarr import (
    CommandBook,
    CommandResponse,
    EventBook,
    EventPage,
    Query,
)
from angzarr_client.builder import (
    CommandBuilder,
    QueryBuilder,
    command,
    command_new,
    query,
    query_domain,
)
from angzarr_client.errors import InvalidArgumentError, InvalidTimestampError
from angzarr_client.helpers import proto_to_uuid


class TestCommandBuilder:
    """Tests for CommandBuilder."""

    def _mock_aggregate_client(self) -> Mock:
        """Create a mock AggregateClient."""
        client = Mock()
        client.handle = Mock(return_value=CommandResponse())
        return client

    def test_build_minimal(self) -> None:
        """Build with minimal required fields."""
        client = self._mock_aggregate_client()
        root = PyUUID("12345678-1234-5678-1234-567812345678")
        msg = StringValue(value="test")

        builder = CommandBuilder(client, "orders", root)
        builder.with_command("type.googleapis.com/test.CreateOrder", msg)
        book = builder.build()

        assert book.cover.domain == "orders"
        assert proto_to_uuid(book.cover.root) == root
        assert len(book.pages) == 1
        assert book.pages[0].sequence == 0
        # Auto-generated correlation ID
        assert book.cover.correlation_id != ""

    def test_build_without_root(self) -> None:
        """Build command for new aggregate (no root)."""
        client = self._mock_aggregate_client()
        msg = StringValue(value="test")

        builder = CommandBuilder(client, "orders")
        builder.with_command("type.googleapis.com/test.CreateOrder", msg)
        book = builder.build()

        assert book.cover.domain == "orders"
        assert not book.cover.HasField("root")

    def test_with_correlation_id(self) -> None:
        """Build with explicit correlation ID."""
        client = self._mock_aggregate_client()
        msg = StringValue(value="test")

        builder = (
            CommandBuilder(client, "orders")
            .with_correlation_id("my-corr-123")
            .with_command("type/Cmd", msg)
        )
        book = builder.build()

        assert book.cover.correlation_id == "my-corr-123"

    def test_with_sequence(self) -> None:
        """Build with specific sequence number."""
        client = self._mock_aggregate_client()
        msg = StringValue(value="test")

        builder = (
            CommandBuilder(client, "orders")
            .with_sequence(5)
            .with_command("type/Cmd", msg)
        )
        book = builder.build()

        assert book.pages[0].sequence == 5

    def test_build_without_type_url_raises(self) -> None:
        """Build without type_url raises InvalidArgumentError."""
        client = self._mock_aggregate_client()
        builder = CommandBuilder(client, "orders")

        with pytest.raises(InvalidArgumentError) as exc_info:
            builder.build()
        assert "type_url" in str(exc_info.value)

    def test_build_without_payload_raises(self) -> None:
        """Build with type_url but no payload raises."""
        client = self._mock_aggregate_client()
        builder = CommandBuilder(client, "orders")
        builder._type_url = "type/Cmd"

        with pytest.raises(InvalidArgumentError) as exc_info:
            builder.build()
        assert "payload" in str(exc_info.value)

    def test_build_propagates_stored_error(self) -> None:
        """Build raises stored error if present."""
        client = self._mock_aggregate_client()
        builder = CommandBuilder(client, "orders")
        builder._err = ValueError("something went wrong")

        with pytest.raises(ValueError):
            builder.build()

    def test_execute_calls_handle(self) -> None:
        """Execute builds and calls client.handle."""
        client = self._mock_aggregate_client()
        expected_response = CommandResponse()
        # CommandResponse contains events field, not correlation_id
        expected_response.events.next_sequence = 5
        client.handle.return_value = expected_response

        msg = StringValue(value="test")
        builder = (
            CommandBuilder(client, "orders")
            .with_command("type/Cmd", msg)
        )
        response = builder.execute()

        client.handle.assert_called_once()
        assert response.events.next_sequence == 5

    def test_fluent_chaining(self) -> None:
        """Methods return self for chaining."""
        client = self._mock_aggregate_client()
        msg = StringValue(value="test")
        root = PyUUID("12345678-1234-5678-1234-567812345678")

        result = (
            CommandBuilder(client, "orders", root)
            .with_correlation_id("corr")
            .with_sequence(5)
            .with_command("type/Cmd", msg)
        )

        assert isinstance(result, CommandBuilder)


class TestQueryBuilder:
    """Tests for QueryBuilder."""

    def _mock_query_client(self) -> Mock:
        """Create a mock QueryClient."""
        client = Mock()
        book = EventBook()
        book.next_sequence = 10
        client.get_event_book = Mock(return_value=book)
        client.get_events = Mock(return_value=[book])
        return client

    def test_build_with_root(self) -> None:
        """Build query for specific aggregate."""
        client = self._mock_query_client()
        root = PyUUID("12345678-1234-5678-1234-567812345678")

        builder = QueryBuilder(client, "orders", root)
        query = builder.build()

        assert query.cover.domain == "orders"
        assert proto_to_uuid(query.cover.root) == root

    def test_build_by_correlation_id(self) -> None:
        """Build query by correlation ID."""
        client = self._mock_query_client()

        builder = QueryBuilder(client, "orders")
        builder.by_correlation_id("corr-abc")
        query = builder.build()

        assert query.cover.correlation_id == "corr-abc"
        assert not query.cover.HasField("root")

    def test_by_correlation_id_clears_root(self) -> None:
        """by_correlation_id clears the root."""
        client = self._mock_query_client()
        root = PyUUID("12345678-1234-5678-1234-567812345678")

        builder = QueryBuilder(client, "orders", root)
        builder.by_correlation_id("corr-abc")
        query = builder.build()

        assert query.cover.correlation_id == "corr-abc"
        assert not query.cover.HasField("root")

    def test_with_edition(self) -> None:
        """Build query with specific edition."""
        client = self._mock_query_client()

        builder = QueryBuilder(client, "orders")
        builder.with_edition("branch-a")
        query = builder.build()

        assert query.cover.edition.name == "branch-a"

    def test_range_lower_only(self) -> None:
        """Build query with lower bound range."""
        client = self._mock_query_client()

        builder = QueryBuilder(client, "orders")
        builder.range(5)
        query = builder.build()

        assert query.range.lower == 5

    def test_range_to(self) -> None:
        """Build query with both range bounds."""
        client = self._mock_query_client()

        builder = QueryBuilder(client, "orders")
        builder.range_to(5, 10)
        query = builder.build()

        assert query.range.lower == 5
        assert query.range.upper == 10

    def test_as_of_sequence(self) -> None:
        """Build temporal query by sequence."""
        client = self._mock_query_client()

        builder = QueryBuilder(client, "orders")
        builder.as_of_sequence(42)
        query = builder.build()

        assert query.temporal.as_of_sequence == 42

    def test_as_of_time_valid(self) -> None:
        """Build temporal query by time."""
        client = self._mock_query_client()

        builder = QueryBuilder(client, "orders")
        builder.as_of_time("2024-01-15T10:30:00Z")
        query = builder.build()

        assert query.temporal.as_of_time.seconds > 0

    def test_as_of_time_invalid_stores_error(self) -> None:
        """Invalid timestamp stores error for later."""
        client = self._mock_query_client()

        builder = QueryBuilder(client, "orders")
        builder.as_of_time("not-a-timestamp")

        with pytest.raises(InvalidTimestampError):
            builder.build()

    def test_build_propagates_stored_error(self) -> None:
        """Build raises stored error if present."""
        client = self._mock_query_client()
        builder = QueryBuilder(client, "orders")
        builder._err = ValueError("something went wrong")

        with pytest.raises(ValueError):
            builder.build()

    def test_get_event_book(self) -> None:
        """get_event_book executes query."""
        client = self._mock_query_client()
        expected_book = EventBook()
        expected_book.next_sequence = 42
        client.get_event_book.return_value = expected_book

        builder = QueryBuilder(client, "orders")
        result = builder.get_event_book()

        client.get_event_book.assert_called_once()
        assert result.next_sequence == 42

    def test_get_events(self) -> None:
        """get_events returns list of books."""
        client = self._mock_query_client()
        book1 = EventBook()
        book1.next_sequence = 5
        book2 = EventBook()
        book2.next_sequence = 10
        client.get_events.return_value = [book1, book2]

        builder = QueryBuilder(client, "orders")
        result = builder.get_events()

        assert len(result) == 2
        assert result[0].next_sequence == 5
        assert result[1].next_sequence == 10

    def test_get_pages(self) -> None:
        """get_pages extracts pages from book."""
        client = self._mock_query_client()
        book = EventBook()
        # EventPage uses 'num' field in oneof sequence
        book.pages.add(num=1)
        book.pages.add(num=2)
        client.get_event_book.return_value = book

        builder = QueryBuilder(client, "orders")
        result = builder.get_pages()

        assert len(result) == 2
        assert result[0].num == 1
        assert result[1].num == 2

    def test_fluent_chaining(self) -> None:
        """Methods return self for chaining."""
        client = self._mock_query_client()
        root = PyUUID("12345678-1234-5678-1234-567812345678")

        result = (
            QueryBuilder(client, "orders", root)
            .with_edition("v2")
            .range_to(0, 10)
        )

        assert isinstance(result, QueryBuilder)


class TestConvenienceFunctions:
    """Tests for convenience functions."""

    def test_command_creates_builder_with_root(self) -> None:
        """command creates builder for existing aggregate."""
        client = Mock()
        root = PyUUID("12345678-1234-5678-1234-567812345678")

        builder = command(client, "orders", root)

        assert isinstance(builder, CommandBuilder)
        assert builder._domain == "orders"
        assert builder._root == root

    def test_command_new_creates_builder_without_root(self) -> None:
        """command_new creates builder for new aggregate."""
        client = Mock()

        builder = command_new(client, "orders")

        assert isinstance(builder, CommandBuilder)
        assert builder._domain == "orders"
        assert builder._root is None

    def test_query_creates_builder_with_root(self) -> None:
        """query creates builder for specific aggregate."""
        client = Mock()
        root = PyUUID("12345678-1234-5678-1234-567812345678")

        builder = query(client, "orders", root)

        assert isinstance(builder, QueryBuilder)
        assert builder._domain == "orders"
        assert builder._root == root

    def test_query_domain_creates_builder_without_root(self) -> None:
        """query_domain creates builder without root."""
        client = Mock()

        builder = query_domain(client, "orders")

        assert isinstance(builder, QueryBuilder)
        assert builder._domain == "orders"
        assert builder._root is None
