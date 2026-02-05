"""Tests for fulfillment command handlers via CommandRouter."""

import pytest
from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import domains_pb2 as domains
from main import router


def _pack_command(command, domain: str = "fulfillment") -> types.ContextualCommand:
    """Pack a domain command into a ContextualCommand."""
    command_any = AnyProto()
    command_any.Pack(command, type_url_prefix="type.examples/")

    return types.ContextualCommand(
        command=types.CommandBook(
            cover=types.Cover(domain=domain),
            pages=[types.CommandPage(sequence=0, command=command_any)],
        ),
    )


def _pack_command_with_events(
    command, prior_events: types.EventBook, domain: str = "fulfillment",
) -> types.ContextualCommand:
    """Pack a domain command with prior events."""
    command_any = AnyProto()
    command_any.Pack(command, type_url_prefix="type.examples/")

    return types.ContextualCommand(
        command=types.CommandBook(
            cover=types.Cover(domain=domain),
            pages=[types.CommandPage(sequence=0, command=command_any)],
        ),
        events=prior_events,
    )


def _shipment_created_events(order_id: str = "order-123") -> types.EventBook:
    """Create an EventBook with a ShipmentCreated event."""
    event = domains.ShipmentCreated(order_id=order_id, status="pending")
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    return types.EventBook(
        pages=[types.EventPage(num=0, event=event_any)],
    )


def _append_event(prior: types.EventBook, event) -> types.EventBook:
    """Append an event to existing events."""
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    pages = list(prior.pages) + [types.EventPage(num=len(prior.pages), event=event_any)]
    return types.EventBook(pages=pages)


def _picked_events(prior: types.EventBook, picker_id: str = "picker-1") -> types.EventBook:
    """Append an ItemsPicked event to existing events."""
    return _append_event(prior, domains.ItemsPicked(picker_id=picker_id))


def _packed_events(prior: types.EventBook, packer_id: str = "packer-1") -> types.EventBook:
    """Append an ItemsPacked event to existing events."""
    return _append_event(prior, domains.ItemsPacked(packer_id=packer_id))


def _shipped_events(
    prior: types.EventBook, carrier: str = "FedEx", tracking: str = "TRACK-001",
) -> types.EventBook:
    """Append a Shipped event to existing events."""
    return _append_event(prior, domains.Shipped(carrier=carrier, tracking_number=tracking))


class TestCreateShipment:
    def test_create_shipment_success(self):
        cmd = domains.CreateShipment(order_id="order-123")
        resp = router.dispatch(_pack_command(cmd))

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1
        assert resp.events.pages[0].event.type_url.endswith("ShipmentCreated")

        event = domains.ShipmentCreated()
        resp.events.pages[0].event.Unpack(event)
        assert event.order_id == "order-123"
        assert event.status == "pending"

    def test_create_shipment_already_exists(self):
        prior = _shipment_created_events()
        cmd = domains.CreateShipment(order_id="order-456")
        with pytest.raises(CommandRejectedError, match="already exists"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_create_shipment_missing_order_id(self):
        cmd = domains.CreateShipment(order_id="")
        with pytest.raises(CommandRejectedError, match="Order ID is required"):
            router.dispatch(_pack_command(cmd))


class TestMarkPicked:
    def test_mark_picked_success(self):
        prior = _shipment_created_events()
        cmd = domains.MarkPicked(picker_id="picker-1")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.ItemsPicked()
        resp.events.pages[0].event.Unpack(event)
        assert event.picker_id == "picker-1"

    def test_mark_picked_shipment_not_found(self):
        cmd = domains.MarkPicked(picker_id="picker-1")
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_mark_picked_not_pending(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        cmd = domains.MarkPicked(picker_id="picker-2")
        with pytest.raises(CommandRejectedError, match="not pending"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_mark_picked_missing_picker_id(self):
        prior = _shipment_created_events()
        cmd = domains.MarkPicked(picker_id="")
        with pytest.raises(CommandRejectedError, match="Picker ID is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestMarkPacked:
    def test_mark_packed_success(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        cmd = domains.MarkPacked(packer_id="packer-1")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.ItemsPacked()
        resp.events.pages[0].event.Unpack(event)
        assert event.packer_id == "packer-1"

    def test_mark_packed_shipment_not_found(self):
        cmd = domains.MarkPacked(packer_id="packer-1")
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_mark_packed_not_picked(self):
        prior = _shipment_created_events()
        cmd = domains.MarkPacked(packer_id="packer-1")
        with pytest.raises(CommandRejectedError, match="not picked"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_mark_packed_missing_packer_id(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        cmd = domains.MarkPacked(packer_id="")
        with pytest.raises(CommandRejectedError, match="Packer ID is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestShip:
    def test_ship_success(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        prior = _packed_events(prior)
        cmd = domains.Ship(carrier="FedEx", tracking_number="TRACK-001")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.Shipped()
        resp.events.pages[0].event.Unpack(event)
        assert event.carrier == "FedEx"
        assert event.tracking_number == "TRACK-001"

    def test_ship_shipment_not_found(self):
        cmd = domains.Ship(carrier="FedEx", tracking_number="TRACK-001")
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_ship_not_packed(self):
        prior = _shipment_created_events()
        cmd = domains.Ship(carrier="FedEx", tracking_number="TRACK-001")
        with pytest.raises(CommandRejectedError, match="not packed"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_ship_missing_carrier(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        prior = _packed_events(prior)
        cmd = domains.Ship(carrier="", tracking_number="TRACK-001")
        with pytest.raises(CommandRejectedError, match="Carrier is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_ship_missing_tracking_number(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        prior = _packed_events(prior)
        cmd = domains.Ship(carrier="FedEx", tracking_number="")
        with pytest.raises(CommandRejectedError, match="Tracking number is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestRecordDelivery:
    def test_record_delivery_success(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        prior = _packed_events(prior)
        prior = _shipped_events(prior)
        cmd = domains.RecordDelivery(signature="John Doe")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.Delivered()
        resp.events.pages[0].event.Unpack(event)
        assert event.signature == "John Doe"

    def test_record_delivery_without_signature(self):
        prior = _shipment_created_events()
        prior = _picked_events(prior)
        prior = _packed_events(prior)
        prior = _shipped_events(prior)
        cmd = domains.RecordDelivery(signature="")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.Delivered()
        resp.events.pages[0].event.Unpack(event)
        assert event.signature == ""

    def test_record_delivery_shipment_not_found(self):
        cmd = domains.RecordDelivery(signature="John Doe")
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_record_delivery_not_shipped(self):
        prior = _shipment_created_events()
        cmd = domains.RecordDelivery(signature="John Doe")
        with pytest.raises(CommandRejectedError, match="not shipped"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestUnknownCommand:
    def test_unknown_command_raises_value_error(self):
        unknown = AnyProto(type_url="type.examples/UnknownCommand", value=b"")
        ctx = types.ContextualCommand(
            command=types.CommandBook(
                cover=types.Cover(domain="fulfillment"),
                pages=[types.CommandPage(sequence=0, command=unknown)],
            ),
        )
        with pytest.raises(ValueError, match="Unknown command type"):
            router.dispatch(ctx)
