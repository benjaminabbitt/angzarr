"""RecordDelivery command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from state import FulfillmentState
from handlers.exceptions import CommandRejectedError


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
