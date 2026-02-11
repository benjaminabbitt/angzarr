"""Tests for protobuf wrapper classes."""

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
    CommandResponse,
    SyncEventBook,
)
from angzarr_client.wrappers import (
    EventBookW,
    CommandBookW,
    CoverW,
    QueryW,
    EventPageW,
    CommandPageW,
    CommandResponseW,
)
from angzarr_client.helpers import (
    UNKNOWN_DOMAIN,
    DEFAULT_EDITION,
    uuid_to_proto,
    proto_to_uuid,
)


class TestEventBookW:
    """Tests for EventBook wrapper class."""

    def test_constructor_accepts_proto(self) -> None:
        """Wrapper accepts EventBook proto in constructor."""
        proto = EventBook()
        wrapper = EventBookW(proto)
        assert wrapper.proto is proto

    def test_next_sequence_returns_value(self) -> None:
        """next_sequence returns the next_sequence field."""
        proto = EventBook()
        proto.next_sequence = 5
        wrapper = EventBookW(proto)
        assert wrapper.next_sequence() == 5

    def test_next_sequence_default_zero(self) -> None:
        """next_sequence returns 0 for new EventBook."""
        wrapper = EventBookW(EventBook())
        assert wrapper.next_sequence() == 0

    def test_pages_returns_wrapped_list(self) -> None:
        """pages returns event pages as wrapped EventPageW instances."""
        proto = EventBook()
        page1 = EventPage(num=1)
        page2 = EventPage(num=2)
        proto.pages.extend([page1, page2])
        wrapper = EventBookW(proto)
        result = wrapper.pages()
        assert len(result) == 2
        assert isinstance(result[0], EventPageW)
        assert isinstance(result[1], EventPageW)
        assert result[0].proto.num == 1
        assert result[1].proto.num == 2

    def test_pages_returns_empty_list_when_none(self) -> None:
        """pages returns empty list for new EventBook."""
        wrapper = EventBookW(EventBook())
        assert wrapper.pages() == []

    def test_domain_returns_domain_from_cover(self) -> None:
        """domain returns domain from embedded cover."""
        proto = EventBook()
        proto.cover.domain = "orders"
        wrapper = EventBookW(proto)
        assert wrapper.domain() == "orders"

    def test_domain_returns_unknown_when_not_set(self) -> None:
        """domain returns UNKNOWN_DOMAIN when cover not set."""
        wrapper = EventBookW(EventBook())
        assert wrapper.domain() == UNKNOWN_DOMAIN

    def test_correlation_id_returns_value(self) -> None:
        """correlation_id returns value from cover."""
        proto = EventBook()
        proto.cover.correlation_id = "corr-123"
        wrapper = EventBookW(proto)
        assert wrapper.correlation_id() == "corr-123"

    def test_correlation_id_returns_empty_when_not_set(self) -> None:
        """correlation_id returns empty string when not set."""
        wrapper = EventBookW(EventBook())
        assert wrapper.correlation_id() == ""

    def test_has_correlation_id_true(self) -> None:
        """has_correlation_id returns True when set."""
        proto = EventBook()
        proto.cover.correlation_id = "xyz"
        wrapper = EventBookW(proto)
        assert wrapper.has_correlation_id() is True

    def test_has_correlation_id_false(self) -> None:
        """has_correlation_id returns False when not set."""
        wrapper = EventBookW(EventBook())
        assert wrapper.has_correlation_id() is False

    def test_root_uuid_returns_uuid(self) -> None:
        """root_uuid returns Python UUID from cover."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        proto = EventBook()
        proto.cover.root.CopyFrom(uuid_to_proto(test_uuid))
        wrapper = EventBookW(proto)
        assert wrapper.root_uuid() == test_uuid

    def test_root_uuid_returns_none_when_not_set(self) -> None:
        """root_uuid returns None when root not set."""
        wrapper = EventBookW(EventBook())
        assert wrapper.root_uuid() is None

    def test_edition_returns_edition_name(self) -> None:
        """edition returns edition name from cover."""
        proto = EventBook()
        proto.cover.edition.name = "v2"
        wrapper = EventBookW(proto)
        assert wrapper.edition() == "v2"

    def test_edition_returns_default_when_not_set(self) -> None:
        """edition returns DEFAULT_EDITION when not set."""
        wrapper = EventBookW(EventBook())
        assert wrapper.edition() == DEFAULT_EDITION

    def test_routing_key_returns_domain(self) -> None:
        """routing_key returns the domain."""
        proto = EventBook()
        proto.cover.domain = "inventory"
        wrapper = EventBookW(proto)
        assert wrapper.routing_key() == "inventory"

    def test_cache_key_returns_domain_and_root(self) -> None:
        """cache_key returns domain:root_hex format."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        proto = EventBook()
        proto.cover.domain = "orders"
        proto.cover.root.CopyFrom(uuid_to_proto(test_uuid))
        wrapper = EventBookW(proto)
        assert wrapper.cache_key() == f"orders:{test_uuid.bytes.hex()}"

    def test_cover_wrapper_returns_cover_w(self) -> None:
        """cover_wrapper returns a CoverW wrapping the cover."""
        proto = EventBook()
        proto.cover.domain = "test"
        wrapper = EventBookW(proto)
        cover_w = wrapper.cover_wrapper()
        assert isinstance(cover_w, CoverW)
        assert cover_w.domain() == "test"


class TestCommandBookW:
    """Tests for CommandBook wrapper class."""

    def test_constructor_accepts_proto(self) -> None:
        """Wrapper accepts CommandBook proto in constructor."""
        proto = CommandBook()
        wrapper = CommandBookW(proto)
        assert wrapper.proto is proto

    def test_pages_returns_wrapped_list(self) -> None:
        """pages returns command pages as wrapped CommandPageW instances."""
        proto = CommandBook()
        page1 = CommandPage(sequence=1)
        page2 = CommandPage(sequence=2)
        proto.pages.extend([page1, page2])
        wrapper = CommandBookW(proto)
        result = wrapper.pages()
        assert len(result) == 2
        assert isinstance(result[0], CommandPageW)
        assert result[0].sequence() == 1

    def test_pages_returns_empty_list_when_none(self) -> None:
        """pages returns empty list for new CommandBook."""
        wrapper = CommandBookW(CommandBook())
        assert wrapper.pages() == []

    def test_domain_returns_domain_from_cover(self) -> None:
        """domain returns domain from embedded cover."""
        proto = CommandBook()
        proto.cover.domain = "fulfillment"
        wrapper = CommandBookW(proto)
        assert wrapper.domain() == "fulfillment"

    def test_correlation_id_returns_value(self) -> None:
        """correlation_id returns value from cover."""
        proto = CommandBook()
        proto.cover.correlation_id = "cmd-456"
        wrapper = CommandBookW(proto)
        assert wrapper.correlation_id() == "cmd-456"


class TestCoverW:
    """Tests for Cover wrapper class."""

    def test_constructor_accepts_proto(self) -> None:
        """Wrapper accepts Cover proto in constructor."""
        proto = Cover(domain="test")
        wrapper = CoverW(proto)
        assert wrapper.proto is proto

    def test_domain_returns_domain(self) -> None:
        """domain returns the domain field."""
        wrapper = CoverW(Cover(domain="orders"))
        assert wrapper.domain() == "orders"

    def test_domain_returns_unknown_for_empty(self) -> None:
        """domain returns UNKNOWN_DOMAIN for empty domain."""
        wrapper = CoverW(Cover())
        assert wrapper.domain() == UNKNOWN_DOMAIN

    def test_correlation_id_returns_value(self) -> None:
        """correlation_id returns the correlation_id field."""
        wrapper = CoverW(Cover(correlation_id="abc-123"))
        assert wrapper.correlation_id() == "abc-123"

    def test_correlation_id_returns_empty_for_unset(self) -> None:
        """correlation_id returns empty string if not set."""
        wrapper = CoverW(Cover())
        assert wrapper.correlation_id() == ""

    def test_has_correlation_id_true(self) -> None:
        """has_correlation_id returns True when set."""
        wrapper = CoverW(Cover(correlation_id="xyz"))
        assert wrapper.has_correlation_id() is True

    def test_has_correlation_id_false(self) -> None:
        """has_correlation_id returns False when empty."""
        wrapper = CoverW(Cover())
        assert wrapper.has_correlation_id() is False

    def test_root_uuid_returns_uuid(self) -> None:
        """root_uuid returns Python UUID."""
        test_uuid = PyUUID("deadbeef-dead-beef-dead-beefdeadbeef")
        proto = Cover()
        proto.root.CopyFrom(uuid_to_proto(test_uuid))
        wrapper = CoverW(proto)
        assert wrapper.root_uuid() == test_uuid

    def test_root_uuid_returns_none_when_not_set(self) -> None:
        """root_uuid returns None when root not set."""
        wrapper = CoverW(Cover())
        assert wrapper.root_uuid() is None

    def test_root_uuid_returns_none_for_invalid_bytes(self) -> None:
        """root_uuid returns None for invalid UUID bytes."""
        proto = Cover()
        proto.root.value = b"invalid"
        wrapper = CoverW(proto)
        assert wrapper.root_uuid() is None

    def test_root_id_hex_returns_hex(self) -> None:
        """root_id_hex returns hex string."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        proto = Cover()
        proto.root.CopyFrom(uuid_to_proto(test_uuid))
        wrapper = CoverW(proto)
        assert wrapper.root_id_hex() == test_uuid.bytes.hex()

    def test_root_id_hex_returns_empty_when_not_set(self) -> None:
        """root_id_hex returns empty string when root not set."""
        wrapper = CoverW(Cover())
        assert wrapper.root_id_hex() == ""

    def test_edition_returns_name(self) -> None:
        """edition returns edition name."""
        proto = Cover()
        proto.edition.name = "speculative"
        wrapper = CoverW(proto)
        assert wrapper.edition() == "speculative"

    def test_edition_returns_default_when_not_set(self) -> None:
        """edition returns DEFAULT_EDITION when not set."""
        wrapper = CoverW(Cover())
        assert wrapper.edition() == DEFAULT_EDITION

    def test_edition_opt_returns_name(self) -> None:
        """edition_opt returns edition name when set."""
        proto = Cover()
        proto.edition.name = "branch-a"
        wrapper = CoverW(proto)
        assert wrapper.edition_opt() == "branch-a"

    def test_edition_opt_returns_none_when_not_set(self) -> None:
        """edition_opt returns None when not set."""
        wrapper = CoverW(Cover())
        assert wrapper.edition_opt() is None

    def test_routing_key_returns_domain(self) -> None:
        """routing_key returns the domain."""
        wrapper = CoverW(Cover(domain="payments"))
        assert wrapper.routing_key() == "payments"

    def test_cache_key_format(self) -> None:
        """cache_key returns domain:root_hex format."""
        test_uuid = PyUUID("12345678-1234-5678-1234-567812345678")
        proto = Cover(domain="inventory")
        proto.root.CopyFrom(uuid_to_proto(test_uuid))
        wrapper = CoverW(proto)
        expected = f"inventory:{test_uuid.bytes.hex()}"
        assert wrapper.cache_key() == expected


class TestQueryW:
    """Tests for Query wrapper class."""

    def test_constructor_accepts_proto(self) -> None:
        """Wrapper accepts Query proto in constructor."""
        proto = Query()
        wrapper = QueryW(proto)
        assert wrapper.proto is proto

    def test_domain_returns_domain_from_cover(self) -> None:
        """domain returns domain from embedded cover."""
        proto = Query()
        proto.cover.domain = "shipping"
        wrapper = QueryW(proto)
        assert wrapper.domain() == "shipping"

    def test_correlation_id_returns_value(self) -> None:
        """correlation_id returns value from cover."""
        proto = Query()
        proto.cover.correlation_id = "query-789"
        wrapper = QueryW(proto)
        assert wrapper.correlation_id() == "query-789"


class TestEventPageW:
    """Tests for EventPage wrapper class."""

    def test_constructor_accepts_proto(self) -> None:
        """Wrapper accepts EventPage proto in constructor."""
        proto = EventPage(num=5)
        wrapper = EventPageW(proto)
        assert wrapper.proto is proto

    def test_decode_event_returns_message(self) -> None:
        """decode_event returns decoded message when type matches."""
        cover = Cover(domain="test", correlation_id="abc")
        proto = EventPage(num=1)
        proto.event.Pack(cover)
        wrapper = EventPageW(proto)

        result = wrapper.decode_event("Cover", Cover)
        assert result is not None
        assert result.domain == "test"
        assert result.correlation_id == "abc"

    def test_decode_event_returns_none_for_mismatch(self) -> None:
        """decode_event returns None when type doesn't match."""
        cover = Cover(domain="test")
        proto = EventPage(num=1)
        proto.event.Pack(cover)
        wrapper = EventPageW(proto)

        result = wrapper.decode_event("OtherType", Cover)
        assert result is None

    def test_decode_event_returns_none_when_no_event(self) -> None:
        """decode_event returns None when event not set."""
        wrapper = EventPageW(EventPage(num=1))
        result = wrapper.decode_event("Cover", Cover)
        assert result is None


class TestCommandPageW:
    """Tests for CommandPage wrapper class."""

    def test_constructor_accepts_proto(self) -> None:
        """Wrapper accepts CommandPage proto in constructor."""
        proto = CommandPage(sequence=10)
        wrapper = CommandPageW(proto)
        assert wrapper.proto is proto

    def test_sequence_returns_value(self) -> None:
        """sequence returns the sequence field."""
        wrapper = CommandPageW(CommandPage(sequence=42))
        assert wrapper.sequence() == 42


class TestCommandResponseW:
    """Tests for CommandResponse wrapper class."""

    def test_constructor_accepts_proto(self) -> None:
        """Wrapper accepts CommandResponse proto in constructor."""
        proto = CommandResponse()
        wrapper = CommandResponseW(proto)
        assert wrapper.proto is proto

    def test_events_book_returns_wrapped_event_book(self) -> None:
        """events_book returns EventBookW when present."""
        proto = CommandResponse()
        proto.events.next_sequence = 5
        proto.events.pages.add(num=1)
        wrapper = CommandResponseW(proto)
        book = wrapper.events_book()
        assert book is not None
        assert isinstance(book, EventBookW)
        assert book.next_sequence() == 5

    def test_events_book_returns_none_when_not_set(self) -> None:
        """events_book returns None when events not set."""
        wrapper = CommandResponseW(CommandResponse())
        assert wrapper.events_book() is None

    def test_events_returns_wrapped_pages_when_present(self) -> None:
        """events returns event pages as wrapped EventPageW instances."""
        proto = CommandResponse()
        proto.events.pages.add(num=1)
        proto.events.pages.add(num=2)
        wrapper = CommandResponseW(proto)
        result = wrapper.events()
        assert len(result) == 2
        assert isinstance(result[0], EventPageW)
        assert isinstance(result[1], EventPageW)

    def test_events_returns_empty_when_not_set(self) -> None:
        """events returns empty list when events not set."""
        wrapper = CommandResponseW(CommandResponse())
        assert wrapper.events() == []


class TestWrapperAttributeAccess:
    """Tests for delegated attribute access to underlying proto."""

    def test_event_book_w_delegates_proto_fields(self) -> None:
        """EventBookW allows access to proto fields via wrapper."""
        proto = EventBook()
        proto.next_sequence = 10
        wrapper = EventBookW(proto)
        # Direct proto access still works
        assert wrapper.proto.next_sequence == 10

    def test_cover_w_delegates_proto_fields(self) -> None:
        """CoverW allows access to proto fields via wrapper."""
        proto = Cover(domain="test", correlation_id="abc")
        wrapper = CoverW(proto)
        # Direct proto access still works
        assert wrapper.proto.domain == "test"
        assert wrapper.proto.correlation_id == "abc"
