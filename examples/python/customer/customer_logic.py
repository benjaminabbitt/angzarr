"""Customer bounded context business logic.

Contains command handlers for customer lifecycle and loyalty points management.
"""

from datetime import datetime, timezone

import structlog
from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from state import next_sequence, rebuild_state


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


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


class CustomerLogic:
    """Business logic for Customer aggregate.

    Legacy class-based interface for Python FFI.
    """

    DOMAIN = "customer"

    def handle(self, contextual_command: angzarr.ContextualCommand) -> angzarr.EventBook:
        """Process a command and return resulting events."""
        command_book = contextual_command.command
        prior_events = contextual_command.events if contextual_command.HasField("events") else None

        # Rebuild current state from events
        state = rebuild_state(prior_events)
        seq = next_sequence(prior_events)

        # Get the command from the first page
        if not command_book.pages:
            raise ValueError("CommandBook has no pages")

        command_page = command_book.pages[0]
        command_any = command_page.command

        # Dummy logger for class-based interface
        log = structlog.get_logger().bind(domain=self.DOMAIN)

        # Route to appropriate handler based on command type
        try:
            if command_any.type_url.endswith("CreateCustomer"):
                return handle_create_customer(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("AddLoyaltyPoints"):
                return handle_add_loyalty_points(command_book, command_any, state, seq, log)
            elif command_any.type_url.endswith("RedeemLoyaltyPoints"):
                return handle_redeem_loyalty_points(command_book, command_any, state, seq, log)
            else:
                raise ValueError(f"Unknown command type: {command_any.type_url}")
        except CommandRejectedError as e:
            raise ValueError(str(e)) from e


# Entry point for angzarr Python FFI
def handle(contextual_command_bytes: bytes) -> bytes:
    """Handle a contextual command and return event book bytes."""
    contextual_command = angzarr.ContextualCommand()
    contextual_command.ParseFromString(contextual_command_bytes)

    logic = CustomerLogic()
    event_book = logic.handle(contextual_command)

    return event_book.SerializeToString()


def get_domains() -> list[str]:
    """Return list of domains this logic handles."""
    return [CustomerLogic.DOMAIN]
