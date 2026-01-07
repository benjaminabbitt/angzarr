"""Customer bounded context business logic.

Handles customer lifecycle and loyalty points management.
"""

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp
from datetime import datetime, timezone

# Import generated proto (will be copied from generated/)
from evented.proto import evented_pb2 as evented
from proto import domains_pb2 as domains


class CustomerLogic:
    """Business logic for Customer aggregate."""

    DOMAIN = "customer"

    def handle(self, contextual_command: evented.ContextualCommand) -> evented.EventBook:
        """Process a command and return resulting events."""
        command_book = contextual_command.command
        prior_events = contextual_command.events

        # Rebuild current state from events
        state = self._rebuild_state(prior_events)

        # Get the command from the first page
        if not command_book.pages:
            raise ValueError("CommandBook has no pages")

        command_page = command_book.pages[0]
        command_any = command_page.command

        # Route to appropriate handler based on command type
        if command_any.type_url.endswith("CreateCustomer"):
            return self._handle_create_customer(command_book, command_any, state)
        elif command_any.type_url.endswith("AddLoyaltyPoints"):
            return self._handle_add_loyalty_points(command_book, command_any, state)
        elif command_any.type_url.endswith("RedeemLoyaltyPoints"):
            return self._handle_redeem_loyalty_points(command_book, command_any, state)
        else:
            raise ValueError(f"Unknown command type: {command_any.type_url}")

    def _rebuild_state(self, event_book: evented.EventBook | None) -> domains.CustomerState:
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

    def _handle_create_customer(
        self,
        command_book: evented.CommandBook,
        command_any: Any,
        state: domains.CustomerState,
    ) -> evented.EventBook:
        """Handle CreateCustomer command."""
        # Validate: customer shouldn't already exist
        if state.name:
            raise ValueError("Customer already exists")

        cmd = domains.CreateCustomer()
        command_any.Unpack(cmd)

        # Validate command
        if not cmd.name:
            raise ValueError("Customer name is required")
        if not cmd.email:
            raise ValueError("Customer email is required")

        # Create event
        event = domains.CustomerCreated(
            name=cmd.name,
            email=cmd.email,
            created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        )

        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.examples/")

        return evented.EventBook(
            cover=command_book.cover,
            pages=[
                evented.EventPage(
                    num=0,
                    event=event_any,
                    created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
                )
            ],
        )

    def _handle_add_loyalty_points(
        self,
        command_book: evented.CommandBook,
        command_any: Any,
        state: domains.CustomerState,
    ) -> evented.EventBook:
        """Handle AddLoyaltyPoints command."""
        # Validate: customer must exist
        if not state.name:
            raise ValueError("Customer does not exist")

        cmd = domains.AddLoyaltyPoints()
        command_any.Unpack(cmd)

        # Validate command
        if cmd.points <= 0:
            raise ValueError("Points must be positive")

        new_balance = state.loyalty_points + cmd.points

        # Create event
        event = domains.LoyaltyPointsAdded(
            points=cmd.points,
            new_balance=new_balance,
            reason=cmd.reason,
        )

        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.examples/")

        return evented.EventBook(
            cover=command_book.cover,
            pages=[
                evented.EventPage(
                    num=0,
                    event=event_any,
                    created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
                )
            ],
        )

    def _handle_redeem_loyalty_points(
        self,
        command_book: evented.CommandBook,
        command_any: Any,
        state: domains.CustomerState,
    ) -> evented.EventBook:
        """Handle RedeemLoyaltyPoints command."""
        # Validate: customer must exist
        if not state.name:
            raise ValueError("Customer does not exist")

        cmd = domains.RedeemLoyaltyPoints()
        command_any.Unpack(cmd)

        # Validate command
        if cmd.points <= 0:
            raise ValueError("Points must be positive")
        if cmd.points > state.loyalty_points:
            raise ValueError(f"Insufficient points: have {state.loyalty_points}, need {cmd.points}")

        new_balance = state.loyalty_points - cmd.points

        # Create event
        event = domains.LoyaltyPointsRedeemed(
            points=cmd.points,
            new_balance=new_balance,
            redemption_type=cmd.redemption_type,
        )

        event_any = Any()
        event_any.Pack(event, type_url_prefix="type.examples/")

        return evented.EventBook(
            cover=command_book.cover,
            pages=[
                evented.EventPage(
                    num=0,
                    event=event_any,
                    created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
                )
            ],
        )


# Entry point for evented-rs Python FFI
def handle(contextual_command_bytes: bytes) -> bytes:
    """Handle a contextual command and return event book bytes."""
    contextual_command = evented.ContextualCommand()
    contextual_command.ParseFromString(contextual_command_bytes)

    logic = CustomerLogic()
    event_book = logic.handle(contextual_command)

    return event_book.SerializeToString()


def get_domains() -> list[str]:
    """Return list of domains this logic handles."""
    return [CustomerLogic.DOMAIN]
