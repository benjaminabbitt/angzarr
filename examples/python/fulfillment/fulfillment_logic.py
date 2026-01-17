"""Fulfillment command handlers."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from state import FulfillmentState


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


def handle_create_shipment(command_book, command_any, state: FulfillmentState, seq: int, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Shipment already exists")

    cmd = domains.CreateShipment()
    command_any.Unpack(cmd)

    if not cmd.order_id:
        raise CommandRejectedError("Order ID is required")

    log.info("creating_shipment", order_id=cmd.order_id)

    event = domains.ShipmentCreated(
        order_id=cmd.order_id,
        status="pending",
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_mark_picked(command_book, command_any, state: FulfillmentState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Shipment does not exist")
    if not state.is_pending():
        raise CommandRejectedError(f"Cannot pick items in {state.status} state")

    cmd = domains.MarkPicked()
    command_any.Unpack(cmd)

    if not cmd.picker_id:
        raise CommandRejectedError("Picker ID is required")

    log.info("marking_picked", picker_id=cmd.picker_id)

    event = domains.ItemsPicked(
        picker_id=cmd.picker_id,
        picked_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_mark_packed(command_book, command_any, state: FulfillmentState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Shipment does not exist")
    if not state.is_picking():
        raise CommandRejectedError(f"Cannot pack items in {state.status} state")

    cmd = domains.MarkPacked()
    command_any.Unpack(cmd)

    if not cmd.packer_id:
        raise CommandRejectedError("Packer ID is required")

    log.info("marking_packed", packer_id=cmd.packer_id)

    event = domains.ItemsPacked(
        packer_id=cmd.packer_id,
        packed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_ship(command_book, command_any, state: FulfillmentState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Shipment does not exist")
    if not state.is_packing():
        raise CommandRejectedError(f"Cannot ship in {state.status} state")

    cmd = domains.Ship()
    command_any.Unpack(cmd)

    if not cmd.carrier:
        raise CommandRejectedError("Carrier is required")
    if not cmd.tracking_number:
        raise CommandRejectedError("Tracking number is required")

    log.info("shipping", carrier=cmd.carrier, tracking_number=cmd.tracking_number)

    event = domains.Shipped(
        carrier=cmd.carrier,
        tracking_number=cmd.tracking_number,
        shipped_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_record_delivery(command_book, command_any, state: FulfillmentState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Shipment does not exist")
    if not state.is_shipped():
        raise CommandRejectedError(f"Cannot record delivery in {state.status} state")

    cmd = domains.RecordDelivery()
    command_any.Unpack(cmd)

    log.info("recording_delivery", signature=cmd.signature)

    event = domains.Delivered(
        signature=cmd.signature,
        delivered_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
