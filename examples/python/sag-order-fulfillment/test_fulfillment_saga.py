"""Tests for fulfillment saga prepare and execute logic."""

import pytest
from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from proto import fulfillment_pb2 as fulfillment
from proto import order_pb2 as order

from main import prepare, execute, TARGET_DOMAIN


ROOT_BYTES = bytes.fromhex("deadbeef01020304")


def _make_source_with_event(type_url: str) -> types.EventBook:
    """Create an EventBook with a single event."""
    root = types.UUID(value=ROOT_BYTES)
    event_any = AnyProto(type_url=type_url, value=b"")
    return types.EventBook(
        cover=types.Cover(domain="order", root=root, correlation_id="corr-123"),
        pages=[types.EventPage(num=0, event=event_any)],
    )


def _make_order_completed_source() -> types.EventBook:
    """Create an EventBook with an OrderCompleted event."""
    root = types.UUID(value=ROOT_BYTES)
    event = order.OrderCompleted()
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    return types.EventBook(
        cover=types.Cover(domain="order", root=root, correlation_id="corr-123"),
        pages=[types.EventPage(num=0, event=event_any)],
    )


class TestPrepare:
    def test_order_completed_returns_destination_cover(self):
        source = _make_source_with_event("type.examples/OrderCompleted")
        destinations = prepare(source)

        assert len(destinations) == 1
        assert destinations[0].domain == TARGET_DOMAIN
        assert destinations[0].root.value == ROOT_BYTES

    def test_no_events_returns_empty(self):
        source = types.EventBook()
        assert prepare(source) == []

    def test_non_matching_event_returns_empty(self):
        source = _make_source_with_event("type.examples/OrderCreated")
        assert prepare(source) == []

    def test_no_root_returns_empty(self):
        event_any = AnyProto(type_url="type.examples/OrderCompleted", value=b"")
        source = types.EventBook(
            cover=types.Cover(domain="order"),
            pages=[types.EventPage(num=0, event=event_any)],
        )
        assert prepare(source) == []


class TestExecute:
    def test_order_completed_produces_create_shipment(self):
        source = _make_order_completed_source()
        commands = execute(source, [])

        assert len(commands) == 1
        cmd_book = commands[0]
        assert cmd_book.cover.domain == TARGET_DOMAIN
        assert cmd_book.cover.correlation_id == "corr-123"
        assert len(cmd_book.pages) == 1

        cmd = fulfillment.CreateShipment()
        cmd_book.pages[0].command.Unpack(cmd)
        assert cmd.order_id == ROOT_BYTES.hex()

    def test_uses_destination_state_for_sequence(self):
        source = _make_order_completed_source()
        dest = types.EventBook(
            pages=[types.EventPage(), types.EventPage(), types.EventPage()],
        )
        commands = execute(source, [dest])

        assert commands[0].pages[0].sequence == 3

    def test_no_destinations_uses_sequence_zero(self):
        source = _make_order_completed_source()
        commands = execute(source, [])

        assert commands[0].pages[0].sequence == 0

    def test_empty_source_returns_empty(self):
        source = types.EventBook()
        assert execute(source, []) == []

    def test_non_matching_event_returns_empty(self):
        source = _make_source_with_event("type.examples/OrderCreated")
        assert execute(source, []) == []
