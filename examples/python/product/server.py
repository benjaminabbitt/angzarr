"""Product bounded context gRPC server.

Handles product catalog management.
"""

import os
from concurrent import futures
from datetime import datetime, timezone

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

DOMAIN = "product"


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> domains.ProductState:
    state = domains.ProductState()

    if event_book is None or not event_book.pages:
        return state

    if event_book.snapshot and event_book.snapshot.state:
        state.ParseFromString(event_book.snapshot.state.value)

    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("ProductCreated"):
            event = domains.ProductCreated()
            page.event.Unpack(event)
            state.sku = event.sku
            state.name = event.name
            state.description = event.description
            state.price_cents = event.price_cents
            state.status = "active"

        elif page.event.type_url.endswith("ProductUpdated"):
            event = domains.ProductUpdated()
            page.event.Unpack(event)
            state.name = event.name
            state.description = event.description

        elif page.event.type_url.endswith("PriceSet"):
            event = domains.PriceSet()
            page.event.Unpack(event)
            state.price_cents = event.new_price_cents

        elif page.event.type_url.endswith("ProductDiscontinued"):
            state.status = "discontinued"

    return state


def handle_create_product(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    if state.sku:
        raise CommandRejectedError("Product already exists")

    cmd = domains.CreateProduct()
    command_any.Unpack(cmd)

    if not cmd.sku:
        raise CommandRejectedError("SKU is required")
    if not cmd.name:
        raise CommandRejectedError("Name is required")
    if cmd.price_cents < 0:
        raise CommandRejectedError("Price cannot be negative")

    log.info("creating_product", sku=cmd.sku, name=cmd.name, price_cents=cmd.price_cents)

    event = domains.ProductCreated(
        sku=cmd.sku,
        name=cmd.name,
        description=cmd.description,
        price_cents=cmd.price_cents,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[
            angzarr.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )


def handle_update_product(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    if not state.sku:
        raise CommandRejectedError("Product does not exist")
    if state.status == "discontinued":
        raise CommandRejectedError("Cannot update discontinued product")

    cmd = domains.UpdateProduct()
    command_any.Unpack(cmd)

    if not cmd.name:
        raise CommandRejectedError("Name is required")

    log.info("updating_product", name=cmd.name)

    event = domains.ProductUpdated(
        name=cmd.name,
        description=cmd.description,
        updated_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[
            angzarr.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )


def handle_set_price(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    if not state.sku:
        raise CommandRejectedError("Product does not exist")
    if state.status == "discontinued":
        raise CommandRejectedError("Cannot change price of discontinued product")

    cmd = domains.SetPrice()
    command_any.Unpack(cmd)

    if cmd.new_price_cents < 0:
        raise CommandRejectedError("Price cannot be negative")

    log.info("setting_price", old_price=state.price_cents, new_price=cmd.new_price_cents)

    event = domains.PriceSet(
        new_price_cents=cmd.new_price_cents,
        previous_price_cents=state.price_cents,
        set_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[
            angzarr.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )


def handle_discontinue(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.ProductState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    if not state.sku:
        raise CommandRejectedError("Product does not exist")
    if state.status == "discontinued":
        raise CommandRejectedError("Product already discontinued")

    cmd = domains.Discontinue()
    command_any.Unpack(cmd)

    log.info("discontinuing_product", reason=cmd.reason)

    event = domains.ProductDiscontinued(
        reason=cmd.reason,
        discontinued_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[
            angzarr.EventPage(
                num=seq,
                event=event_any,
                created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
            )
        ],
    )


class BusinessLogicServicer(angzarr_pb2_grpc.BusinessLogicServicer):
    def __init__(self) -> None:
        self.log = logger.bind(domain=DOMAIN, service="business_logic")

    def Handle(
        self,
        request: angzarr.ContextualCommand,
        context: grpc.ServicerContext,
    ) -> angzarr.EventBook:
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
            if command_any.type_url.endswith("CreateProduct"):
                return handle_create_product(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("UpdateProduct"):
                return handle_update_product(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("SetPrice"):
                return handle_set_price(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("Discontinue"):
                return handle_discontinue(command_book, command_any, state, seq, log)
            else:
                context.abort(
                    grpc.StatusCode.INVALID_ARGUMENT,
                    f"Unknown command type: {command_any.type_url}",
                )
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    port = os.environ.get("PORT", "50301")

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
