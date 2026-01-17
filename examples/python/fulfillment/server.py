"""Fulfillment bounded context gRPC server.

Handles shipment lifecycle (pick, pack, ship, deliver).
"""

import os
from concurrent import futures
from datetime import datetime, timezone
from dataclasses import dataclass

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from angzarr import angzarr_pb2_grpc
from proto import domains_pb2 as domains

structlog.configure(
    processors=[
        structlog.stdlib.add_log_level,
        structlog.processors.TimeStamper(fmt="iso"),
        structlog.processors.JSONRenderer(),
    ],
    wrapper_class=structlog.make_filtering_bound_logger(0),
    context_class=dict,
    logger_factory=structlog.PrintLoggerFactory(),
)

logger = structlog.get_logger()

DOMAIN = "fulfillment"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


@dataclass
class FulfillmentState:
    order_id: str = ""
    status: str = ""  # "pending", "picking", "packing", "shipped", "delivered"
    tracking_number: str = ""
    carrier: str = ""
    picker_id: str = ""
    packer_id: str = ""
    signature: str = ""

    def exists(self) -> bool:
        return bool(self.order_id)

    def is_pending(self) -> bool:
        return self.status == "pending"

    def is_picking(self) -> bool:
        return self.status == "picking"

    def is_packing(self) -> bool:
        return self.status == "packing"

    def is_shipped(self) -> bool:
        return self.status == "shipped"

    def is_delivered(self) -> bool:
        return self.status == "delivered"


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> FulfillmentState:
    state = FulfillmentState()

    if event_book is None or not event_book.pages:
        return state

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("ShipmentCreated"):
            event = domains.ShipmentCreated()
            page.event.Unpack(event)
            state.order_id = event.order_id
            state.status = event.status

        elif page.event.type_url.endswith("ItemsPicked"):
            event = domains.ItemsPicked()
            page.event.Unpack(event)
            state.picker_id = event.picker_id
            state.status = "picking"

        elif page.event.type_url.endswith("ItemsPacked"):
            event = domains.ItemsPacked()
            page.event.Unpack(event)
            state.packer_id = event.packer_id
            state.status = "packing"

        elif page.event.type_url.endswith("Shipped"):
            event = domains.Shipped()
            page.event.Unpack(event)
            state.carrier = event.carrier
            state.tracking_number = event.tracking_number
            state.status = "shipped"

        elif page.event.type_url.endswith("Delivered"):
            event = domains.Delivered()
            page.event.Unpack(event)
            state.signature = event.signature
            state.status = "delivered"

    return state


def handle_create_shipment(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_mark_picked(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_mark_packed(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_ship(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


def handle_record_delivery(command_book, command_any, state, seq, log) -> angzarr.EventBook:
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


class BusinessLogicServicer(angzarr_pb2_grpc.BusinessLogicServicer):
    def __init__(self) -> None:
        self.log = logger.bind(domain=DOMAIN, service="business_logic")

    def Handle(self, request: angzarr.ContextualCommand, context: grpc.ServicerContext) -> angzarr.EventBook:
        command_book = request.command
        prior_events = request.events if request.HasField("events") else None

        if not command_book.pages:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, "CommandBook has no pages")

        command_page = command_book.pages[0]
        command_any = command_page.command

        state = rebuild_state(prior_events)
        seq = next_sequence(prior_events)

        log = self.log.bind(command_type=command_any.type_url.split(".")[-1])

        try:
            if command_any.type_url.endswith("CreateShipment"):
                return handle_create_shipment(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("MarkPicked"):
                return handle_mark_picked(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("MarkPacked"):
                return handle_mark_packed(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("Ship"):
                return handle_ship(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("RecordDelivery"):
                return handle_record_delivery(command_book, command_any, state, seq, log)
            else:
                context.abort(grpc.StatusCode.INVALID_ARGUMENT, f"Unknown command type: {command_any.type_url}")
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    port = os.environ.get("PORT", "50305")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_BusinessLogicServicer_to_server(BusinessLogicServicer(), server)

    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info("server_started", domain=DOMAIN, port=port)

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
