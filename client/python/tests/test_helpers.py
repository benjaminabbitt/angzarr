"""Tests for helper functions."""

from datetime import datetime, timezone
from uuid import UUID as PyUUID

import pytest
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import (
    UUID,
    Cover,
    Edition,
    DomainDivergence,
    EventBook,
    EventPage,
    CommandBook,
    CommandPage,
    Query,
    SequenceRange,
    TemporalQuery,
)
from angzarr_client.helpers import (
    # Constants
    UNKNOWN_DOMAIN,
    WILDCARD_DOMAIN,
    DEFAULT_EDITION,
    META_ANGZARR_DOMAIN,
    PROJECTION_DOMAIN_PREFIX,
    CORRELATION_ID_HEADER,
    TYPE_URL_PREFIX,
    # Cover functions
    cover_of,
    domain,
    correlation_id,
    has_correlation_id,
    root_uuid,
    root_id_hex,
    edition,
    edition_opt,
    routing_key,
    cache_key,
    # UUID conversion
    uuid_to_proto,
    proto_to_uuid,
    # Edition helpers
    main_timeline,
    implicit_edition,
    explicit_edition,
    is_main_timeline,
    divergence_for,
    # EventBook helpers
    next_sequence,
    event_pages,
    # CommandBook helpers
    command_pages,
    # CommandResponse helpers
    events_from_response,
    # Type URL helpers
    type_url,
    type_name_from_url,
    type_url_matches,
    # Timestamp helpers
    now,
    parse_timestamp,
    # Event decoding
    decode_event,
    # Construction helpers
    new_cover,
    new_command_page,
    new_command_book,
    range_selection,
    temporal_by_sequence,
    temporal_by_time,
)
from angzarr_client.errors import InvalidTimestampError


class TestConstants:
    """Tests for module constants."""

    def test_unknown_domain(self) -> None:
        assert UNKNOWN_DOMAIN == "unknown"

    def test_wildcard_domain(self) -> None:
        assert WILDCARD_DOMAIN == "*"

    def test_default_edition(self) -> None:
        assert DEFAULT_EDITION == "angzarr"

    def test_meta_domain(self) -> None:
        assert META_ANGZARR_DOMAIN == "_angzarr"

    def test_projection_prefix(self) -> None:
        assert PROJECTION_DOMAIN_PREFIX == "projection:"

    def test_correlation_header(self) -> None:
        assert CORRELATION_ID_HEADER == "x-correlation-id"

    def test_type_url_prefix(self) -> None:
        assert TYPE_URL_PREFIX == "type.googleapis.com/"


class TestCoverOf:
    """Tests for cover_of function."""

    def test_cover_returns_self(self) -> None:
        """Cover object returns itself."""
        cover = Cover(domain="test")
        assert cover_of(cover) is cover

    def test_event_book_returns_cover(self) -> None:
        """EventBook returns its cover."""
        cover = Cover(domain="orders")
        book = EventBook()
        book.cover.CopyFrom(cover)
        result = cover_of(book)
        assert result.domain == "orders"

    def test_command_book_returns_cover(self) -> None:
        """CommandBook returns its cover."""
        cover = Cover(domain="inventory")
        book = CommandBook()
        book.cover.CopyFrom(cover)
        result = cover_of(book)
        assert result.domain == "inventory"

    def test_query_returns_cover(self) -> None:
        """Query returns its cover."""
        cover = Cover(domain="shipping")
        query = Query()
        query.cover.CopyFrom(cover)
        result = cover_of(query)
        assert result.domain == "shipping"

    def test_object_without_cover_returns_none(self) -> None:
        """Object without cover attribute returns None."""
        result = cover_of("not a cover bearer")  # type: ignore
        assert result is None


class TestDomain:
    """Tests for domain function."""

    def test_returns_domain_from_cover(self) -> None:
        """Returns domain from Cover."""
        cover = Cover(domain="payments")
        assert domain(cover) == "payments"

    def test_returns_unknown_for_empty_domain(self) -> None:
        """Returns UNKNOWN_DOMAIN for empty domain."""
        cover = Cover()
        assert domain(cover) == UNKNOWN_DOMAIN

    def test_returns_unknown_for_none(self) -> None:
        """Returns UNKNOWN_DOMAIN for invalid input."""
        assert domain("invalid")  == UNKNOWN_DOMAIN  # type: ignore


class TestCorrelationId:
    """Tests for correlation_id function."""

    def test_returns_correlation_id(self) -> None:
        """Returns correlation_id from Cover."""
        cover = Cover(correlation_id="abc-123")
        assert correlation_id(cover) == "abc-123"

    def test_returns_empty_for_no_correlation(self) -> None:
        """Returns empty string if not set."""
        cover = Cover(domain="test")
        assert correlation_id(cover) == ""

    def test_returns_empty_for_invalid_input(self) -> None:
        """Returns empty string for invalid input."""
        assert correlation_id("invalid") == ""  # type: ignore


class TestHasCorrelationId:
    """Tests for has_correlation_id function."""

    def test_true_when_set(self) -> None:
        """Returns True when correlation_id is set."""
        cover = Cover(correlation_id="xyz")
        assert has_correlation_id(cover) is True

    def test_false_when_empty(self) -> None:
        """Returns False when correlation_id is empty."""
        cover = Cover()
        assert has_correlation_id(cover) is False

    def test_false_for_invalid(self) -> None:
        """Returns False for invalid input."""
        assert has_correlation_id("invalid") is False  # type: ignore


class TestRootUuid:
    """Tests for root_uuid function."""

    def test_returns_uuid(self) -> None:
        """Returns Python UUID from Cover."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        cover = Cover(domain="test")
        cover.root.CopyFrom(uuid_to_proto(test_uuid))
        result = root_uuid(cover)
        assert result == test_uuid

    def test_returns_none_when_no_root(self) -> None:
        """Returns None when root not set."""
        cover = Cover(domain="test")
        assert root_uuid(cover) is None

    def test_returns_none_for_invalid_bytes(self) -> None:
        """Returns None for invalid UUID bytes."""
        cover = Cover(domain="test")
        cover.root.value = b"invalid"  # Not 16 bytes
        assert root_uuid(cover) is None


class TestRootIdHex:
    """Tests for root_id_hex function."""

    def test_returns_hex_string(self) -> None:
        """Returns hex representation of root UUID."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        cover = Cover(domain="test")
        cover.root.CopyFrom(uuid_to_proto(test_uuid))
        result = root_id_hex(cover)
        assert result == test_uuid.bytes.hex()

    def test_returns_empty_when_no_root(self) -> None:
        """Returns empty string when root not set."""
        cover = Cover(domain="test")
        assert root_id_hex(cover) == ""


class TestEdition:
    """Tests for edition function."""

    def test_returns_edition_name(self) -> None:
        """Returns edition name from Cover."""
        cover = Cover(domain="test")
        cover.edition.name = "v2"
        assert edition(cover) == "v2"

    def test_returns_default_when_not_set(self) -> None:
        """Returns DEFAULT_EDITION when not set."""
        cover = Cover(domain="test")
        assert edition(cover) == DEFAULT_EDITION

    def test_returns_default_for_empty_name(self) -> None:
        """Returns DEFAULT_EDITION for empty name."""
        cover = Cover(domain="test")
        cover.edition.name = ""
        assert edition(cover) == DEFAULT_EDITION


class TestEditionOpt:
    """Tests for edition_opt function."""

    def test_returns_edition_name(self) -> None:
        """Returns edition name when set."""
        cover = Cover(domain="test")
        cover.edition.name = "speculative"
        assert edition_opt(cover) == "speculative"

    def test_returns_none_when_not_set(self) -> None:
        """Returns None when edition not set."""
        cover = Cover(domain="test")
        assert edition_opt(cover) is None


class TestRoutingKey:
    """Tests for routing_key function."""

    def test_returns_domain(self) -> None:
        """Routing key is the domain."""
        cover = Cover(domain="orders")
        assert routing_key(cover) == "orders"


class TestCacheKey:
    """Tests for cache_key function."""

    def test_returns_domain_and_root(self) -> None:
        """Cache key combines domain and root hex."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        cover = Cover(domain="orders")
        cover.root.CopyFrom(uuid_to_proto(test_uuid))
        result = cache_key(cover)
        assert result == f"orders:{test_uuid.bytes.hex()}"

    def test_returns_domain_with_empty_root(self) -> None:
        """Cache key with no root has empty suffix."""
        cover = Cover(domain="orders")
        assert cache_key(cover) == "orders:"


class TestUuidConversion:
    """Tests for UUID conversion functions."""

    def test_round_trip(self) -> None:
        """UUID can round-trip through proto."""
        original = PyUUID("deadbeef-dead-beef-dead-beefdeadbeef")
        proto = uuid_to_proto(original)
        result = proto_to_uuid(proto)
        assert result == original

    def test_uuid_to_proto_bytes(self) -> None:
        """uuid_to_proto sets correct bytes."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        proto = uuid_to_proto(test_uuid)
        assert proto.value == test_uuid.bytes


class TestEditionHelpers:
    """Tests for edition helper functions."""

    def test_main_timeline(self) -> None:
        """main_timeline returns default edition."""
        ed = main_timeline()
        assert ed.name == DEFAULT_EDITION
        assert len(ed.divergences) == 0

    def test_implicit_edition(self) -> None:
        """implicit_edition creates named edition without divergences."""
        ed = implicit_edition("branch-a")
        assert ed.name == "branch-a"
        assert len(ed.divergences) == 0

    def test_explicit_edition(self) -> None:
        """explicit_edition creates edition with divergences."""
        divergences = [
            DomainDivergence(domain="orders", sequence=5),
            DomainDivergence(domain="inventory", sequence=10),
        ]
        ed = explicit_edition("branch-b", divergences)
        assert ed.name == "branch-b"
        assert len(ed.divergences) == 2

    def test_is_main_timeline_none(self) -> None:
        """is_main_timeline returns True for None."""
        assert is_main_timeline(None) is True

    def test_is_main_timeline_empty_name(self) -> None:
        """is_main_timeline returns True for empty name."""
        ed = Edition()
        assert is_main_timeline(ed) is True

    def test_is_main_timeline_default(self) -> None:
        """is_main_timeline returns True for default edition."""
        ed = Edition(name=DEFAULT_EDITION)
        assert is_main_timeline(ed) is True

    def test_is_main_timeline_other(self) -> None:
        """is_main_timeline returns False for other editions."""
        ed = Edition(name="speculative")
        assert is_main_timeline(ed) is False

    def test_divergence_for_found(self) -> None:
        """divergence_for returns sequence when found."""
        ed = Edition(
            name="test",
            divergences=[DomainDivergence(domain="orders", sequence=42)],
        )
        assert divergence_for(ed, "orders") == 42

    def test_divergence_for_not_found(self) -> None:
        """divergence_for returns -1 when not found."""
        ed = Edition(
            name="test",
            divergences=[DomainDivergence(domain="orders", sequence=42)],
        )
        assert divergence_for(ed, "inventory") == -1

    def test_divergence_for_none(self) -> None:
        """divergence_for returns -1 for None edition."""
        assert divergence_for(None, "orders") == -1


class TestEventBookHelpers:
    """Tests for EventBook helper functions."""

    def test_next_sequence_returns_value(self) -> None:
        """next_sequence returns the next_sequence field."""
        book = EventBook()
        book.next_sequence = 5
        assert next_sequence(book) == 5

    def test_next_sequence_none_returns_zero(self) -> None:
        """next_sequence returns 0 for None."""
        assert next_sequence(None) == 0  # type: ignore

    def test_event_pages_returns_list(self) -> None:
        """event_pages returns pages as list."""
        book = EventBook()
        page1 = EventPage(sequence=1)
        page2 = EventPage(sequence=2)
        book.pages.extend([page1, page2])
        result = event_pages(book)
        assert len(result) == 2
        assert result[0].sequence == 1
        assert result[1].sequence == 2

    def test_event_pages_none_returns_empty(self) -> None:
        """event_pages returns empty list for None."""
        assert event_pages(None) == []


class TestCommandBookHelpers:
    """Tests for CommandBook helper functions."""

    def test_command_pages_returns_list(self) -> None:
        """command_pages returns pages as list."""
        book = CommandBook()
        page1 = CommandPage(sequence=1)
        page2 = CommandPage(sequence=2)
        book.pages.extend([page1, page2])
        result = command_pages(book)
        assert len(result) == 2
        assert result[0].sequence == 1

    def test_command_pages_none_returns_empty(self) -> None:
        """command_pages returns empty list for None."""
        assert command_pages(None) == []


class TestEventsFromResponse:
    """Tests for events_from_response function."""

    def test_returns_none_for_none_response(self) -> None:
        """Returns empty list for None response."""
        assert events_from_response(None) == []

    def test_returns_empty_for_no_events_field(self) -> None:
        """Returns empty list when events field not set."""
        from angzarr_client.proto.angzarr import CommandResponse
        resp = CommandResponse()
        assert events_from_response(resp) == []

    def test_returns_pages_when_present(self) -> None:
        """Returns event pages when present."""
        from angzarr_client.proto.angzarr import CommandResponse, SyncEventBook
        resp = CommandResponse()
        resp.events.pages.add(sequence=1)
        resp.events.pages.add(sequence=2)
        result = events_from_response(resp)
        assert len(result) == 2


class TestTypeUrlHelpers:
    """Tests for type URL helper functions."""

    def test_type_url_construction(self) -> None:
        """type_url constructs full URL."""
        result = type_url("com.example", "MyMessage")
        assert result == "type.googleapis.com/com.example.MyMessage"

    def test_type_name_from_url_with_dot(self) -> None:
        """type_name_from_url extracts name after last dot."""
        result = type_name_from_url("type.googleapis.com/com.example.MyMessage")
        assert result == "MyMessage"

    def test_type_name_from_url_with_slash(self) -> None:
        """type_name_from_url extracts name after slash if no dot."""
        result = type_name_from_url("prefix/MyMessage")
        assert result == "MyMessage"

    def test_type_name_from_url_plain(self) -> None:
        """type_name_from_url returns input if no separators."""
        result = type_name_from_url("MyMessage")
        assert result == "MyMessage"

    def test_type_url_matches_true(self) -> None:
        """type_url_matches returns True for matching suffix."""
        assert type_url_matches("com.example.OrderCreated", "OrderCreated") is True

    def test_type_url_matches_false(self) -> None:
        """type_url_matches returns False for non-matching suffix."""
        assert type_url_matches("com.example.OrderCreated", "OrderCanceled") is False


class TestTimestampHelpers:
    """Tests for timestamp helper functions."""

    def test_now_returns_timestamp(self) -> None:
        """now returns a Timestamp with current time."""
        before = datetime.now(timezone.utc)
        ts = now()
        after = datetime.now(timezone.utc)
        # Timestamp should be between before and after
        ts_datetime = ts.ToDatetime(tzinfo=timezone.utc)
        assert before <= ts_datetime <= after

    def test_parse_timestamp_valid(self) -> None:
        """parse_timestamp parses valid RFC3339."""
        ts = parse_timestamp("2024-01-15T10:30:00Z")
        assert ts.seconds > 0

    def test_parse_timestamp_with_nanos(self) -> None:
        """parse_timestamp handles nanoseconds."""
        ts = parse_timestamp("2024-01-15T10:30:00.123456789Z")
        assert ts.nanos > 0

    def test_parse_timestamp_invalid_raises(self) -> None:
        """parse_timestamp raises InvalidTimestampError for invalid input."""
        with pytest.raises(InvalidTimestampError):
            parse_timestamp("not-a-timestamp")


class TestDecodeEvent:
    """Tests for decode_event function."""

    def test_returns_none_for_none_page(self) -> None:
        """Returns None for None page."""
        from angzarr_client.proto.angzarr import Cover
        assert decode_event(None, "Cover", Cover) is None

    def test_returns_none_for_no_event_field(self) -> None:
        """Returns None when event field not set."""
        from angzarr_client.proto.angzarr import Cover
        page = EventPage(sequence=1)
        assert decode_event(page, "Cover", Cover) is None

    def test_returns_none_for_type_mismatch(self) -> None:
        """Returns None when type URL doesn't match."""
        from angzarr_client.proto.angzarr import Cover
        page = EventPage(sequence=1)
        page.event.type_url = "type.googleapis.com/some.OtherType"
        page.event.value = b""
        assert decode_event(page, "Cover", Cover) is None

    def test_returns_decoded_message(self) -> None:
        """Returns decoded message when type matches."""
        from angzarr_client.proto.angzarr import Cover
        # Create a cover and pack it
        cover = Cover(domain="test", correlation_id="abc")
        page = EventPage(sequence=1)
        page.event.Pack(cover)

        result = decode_event(page, "Cover", Cover)
        assert result is not None
        assert result.domain == "test"
        assert result.correlation_id == "abc"

    def test_returns_none_for_decode_failure(self) -> None:
        """Returns None when decoding fails."""
        from angzarr_client.proto.angzarr import Cover
        # Create page with matching type URL but invalid data
        page = EventPage(sequence=1)
        page.event.type_url = "type.googleapis.com/angzarr.Cover"
        page.event.value = b"invalid proto data that will fail to decode"
        # Should return None, not raise
        assert decode_event(page, "Cover", Cover) is None


class TestConstructionHelpers:
    """Tests for construction helper functions."""

    def test_new_cover_minimal(self) -> None:
        """new_cover creates cover with required fields."""
        root = PyUUID("12345678-1234-5678-1234-567812345678")
        cover = new_cover("orders", root)
        assert cover.domain == "orders"
        assert proto_to_uuid(cover.root) == root
        assert cover.correlation_id == ""

    def test_new_cover_with_correlation(self) -> None:
        """new_cover accepts correlation_id."""
        root = PyUUID("12345678-1234-5678-1234-567812345678")
        cover = new_cover("orders", root, correlation_id_val="corr-123")
        assert cover.correlation_id == "corr-123"

    def test_new_cover_with_edition(self) -> None:
        """new_cover accepts edition."""
        root = PyUUID("12345678-1234-5678-1234-567812345678")
        ed = implicit_edition("branch-a")
        cover = new_cover("orders", root, edition_val=ed)
        assert cover.edition.name == "branch-a"

    def test_new_command_page(self) -> None:
        """new_command_page creates page with sequence and command."""
        any_msg = ProtoAny(type_url="test/Cmd", value=b"data")
        page = new_command_page(5, any_msg)
        assert page.sequence == 5
        assert page.command.type_url == "test/Cmd"

    def test_new_command_book(self) -> None:
        """new_command_book creates book with cover and pages."""
        root = PyUUID("12345678-1234-5678-1234-567812345678")
        cover = new_cover("orders", root)
        any_msg = ProtoAny(type_url="test/Cmd", value=b"data")
        pages = [new_command_page(0, any_msg)]

        book = new_command_book(cover, pages)
        assert book.cover.domain == "orders"
        assert len(book.pages) == 1
        assert book.pages[0].sequence == 0

    def test_range_selection_lower_only(self) -> None:
        """range_selection with lower bound only."""
        r = range_selection(5)
        assert r.lower == 5
        assert r.upper == 0  # Default

    def test_range_selection_with_upper(self) -> None:
        """range_selection with both bounds."""
        r = range_selection(5, 10)
        assert r.lower == 5
        assert r.upper == 10

    def test_temporal_by_sequence(self) -> None:
        """temporal_by_sequence creates as-of query."""
        tq = temporal_by_sequence(42)
        assert tq.as_of_sequence == 42

    def test_temporal_by_time(self) -> None:
        """temporal_by_time creates time-based query."""
        ts = parse_timestamp("2024-01-15T10:30:00Z")
        tq = temporal_by_time(ts)
        assert tq.as_of_time.seconds == ts.seconds
