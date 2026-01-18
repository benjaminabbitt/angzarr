"""Customer bounded context FFI entry points.

Contains the legacy class-based interface for Python FFI.
"""

import structlog

from angzarr import angzarr_pb2 as angzarr

from handlers.state import next_sequence, rebuild_state
from handlers import (
    CommandRejectedError,
    handle_create_customer,
    handle_add_loyalty_points,
    handle_redeem_loyalty_points,
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
