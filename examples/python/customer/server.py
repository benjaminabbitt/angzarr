"""Customer bounded context gRPC server.

Handles customer lifecycle and loyalty points management.
"""

import os
from concurrent import futures
from datetime import datetime, timezone
from typing import Protocol

import grpc
import structlog
from grpc_health.v1 import health, health_pb2, health_pb2_grpc
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from angzarr import angzarr_pb2_grpc
from proto import domains_pb2 as domains

# Configure structlog
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

DOMAIN = "customer"


class CustomerState(Protocol):
    """Protocol for customer state."""
    name: str
    email: str
    loyalty_points: int
    lifetime_points: int


class StateRebuildError(Exception):
    """Error during state rebuilding."""


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


def next_sequence(event_book: angzarr.EventBook | None) -> int:
    """Return the next event sequence number based on prior events."""
    if event_book is None or not event_book.pages:
        return 0
    return len(event_book.pages)


def rebuild_state(event_book: angzarr.EventBook | None) -> domains.CustomerState:
    """Rebuild customer state from events."""
    state = domains.CustomerState()

    if event_book is None or not event_book.pages:
        return state

    # Start from snapshot if present
    if event_book.snapshot and event_book.snapshot.state:
        state.ParseFromString(event_book.snapshot.state.value)

    # Apply events
    for page in event_book.pages:
        if not page.event:
            continue

        if page.event.type_url.endswith("CustomerCreated"):
            event = domains.CustomerCreated()
            page.event.Unpack(event)
            state.name = event.name
            state.email = event.email

        elif page.event.type_url.endswith("LoyaltyPointsAdded"):
            event = domains.LoyaltyPointsAdded()
            page.event.Unpack(event)
            state.loyalty_points = event.new_balance
            state.lifetime_points += event.points

        elif page.event.type_url.endswith("LoyaltyPointsRedeemed"):
            event = domains.LoyaltyPointsRedeemed()
            page.event.Unpack(event)
            state.loyalty_points = event.new_balance

    return state


def handle_create_customer(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.CustomerState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle CreateCustomer command."""
    if state.name:
        raise CommandRejectedError("Customer already exists")

    cmd = domains.CreateCustomer()
    command_any.Unpack(cmd)

    if not cmd.name:
        raise CommandRejectedError("Customer name is required")
    if not cmd.email:
        raise CommandRejectedError("Customer email is required")

    log.info("creating_customer", name=cmd.name, email=cmd.email)

    event = domains.CustomerCreated(
        name=cmd.name,
        email=cmd.email,
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


def handle_add_loyalty_points(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.CustomerState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle AddLoyaltyPoints command."""
    if not state.name:
        raise CommandRejectedError("Customer does not exist")

    cmd = domains.AddLoyaltyPoints()
    command_any.Unpack(cmd)

    if cmd.points <= 0:
        raise CommandRejectedError("Points must be positive")

    new_balance = state.loyalty_points + cmd.points

    log.info(
        "adding_loyalty_points",
        points=cmd.points,
        new_balance=new_balance,
        reason=cmd.reason,
    )

    event = domains.LoyaltyPointsAdded(
        points=cmd.points,
        new_balance=new_balance,
        reason=cmd.reason,
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


def handle_redeem_loyalty_points(
    command_book: angzarr.CommandBook,
    command_any: Any,
    state: domains.CustomerState,
    seq: int,
    log: structlog.BoundLogger,
) -> angzarr.EventBook:
    """Handle RedeemLoyaltyPoints command."""
    if not state.name:
        raise CommandRejectedError("Customer does not exist")

    cmd = domains.RedeemLoyaltyPoints()
    command_any.Unpack(cmd)

    if cmd.points <= 0:
        raise CommandRejectedError("Points must be positive")
    if cmd.points > state.loyalty_points:
        raise CommandRejectedError(
            f"Insufficient points: have {state.loyalty_points}, need {cmd.points}"
        )

    new_balance = state.loyalty_points - cmd.points

    log.info(
        "redeeming_loyalty_points",
        points=cmd.points,
        new_balance=new_balance,
        redemption_type=cmd.redemption_type,
    )

    event = domains.LoyaltyPointsRedeemed(
        points=cmd.points,
        new_balance=new_balance,
        redemption_type=cmd.redemption_type,
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
    """gRPC service implementation for Customer business logic."""

    def __init__(self) -> None:
        self.log = logger.bind(domain=DOMAIN, service="business_logic")

    def Handle(
        self,
        request: angzarr.ContextualCommand,
        context: grpc.ServicerContext,
    ) -> angzarr.EventBook:
        """Process a command and return resulting events."""
        command_book = request.command
        prior_events = request.events if request.HasField("events") else None

        if not command_book.pages:
            context.abort(grpc.StatusCode.INVALID_ARGUMENT, "CommandBook has no pages")

        command_page = command_book.pages[0]
        command_any = command_page.command

        # Rebuild state from prior events
        state = rebuild_state(prior_events)
        seq = next_sequence(prior_events)

        log = self.log.bind(command_type=command_any.type_url.split(".")[-1])

        try:
            if command_any.type_url.endswith("CreateCustomer"):
                return handle_create_customer(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("AddLoyaltyPoints"):
                return handle_add_loyalty_points(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("RedeemLoyaltyPoints"):
                return handle_redeem_loyalty_points(command_book, command_any, state, seq, log)
            else:
                context.abort(
                    grpc.StatusCode.INVALID_ARGUMENT,
                    f"Unknown command type: {command_any.type_url}",
                )
        except CommandRejectedError as e:
            log.warning("command_rejected", reason=str(e))
            context.abort(grpc.StatusCode.FAILED_PRECONDITION, str(e))


def serve() -> None:
    """Start the gRPC server."""
    port = os.environ.get("PORT", "50052")

    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    angzarr_pb2_grpc.add_BusinessLogicServicer_to_server(BusinessLogicServicer(), server)

    # Register gRPC health service
    health_servicer = health.HealthServicer()
    health_pb2_grpc.add_HealthServicer_to_server(health_servicer, server)
    health_servicer.set("", health_pb2.HealthCheckResponse.SERVING)

    server.add_insecure_port(f"[::]:{port}")

    logger.info("server_started", domain=DOMAIN, port=port)

    server.start()
    server.wait_for_termination()


if __name__ == "__main__":
    serve()
