"""Loyalty Points Saga - Python Implementation.

Listens to TransactionCompleted events and sends AddLoyaltyPoints
commands to the customer domain.
"""

from typing import List
from dataclasses import dataclass

from proto import domains_pb2 as domains


@dataclass
class CommandBook:
    """Represents a command to be sent."""
    domain: str
    root_id: bytes
    command_type: str
    command_data: bytes


class LoyaltyPointsSaga:
    """Saga that awards loyalty points when transactions complete."""

    def __init__(self):
        self.name = "loyalty_points"

    def domains(self) -> List[str]:
        """Return the domains this saga listens to."""
        return ["transaction"]

    def is_synchronous(self) -> bool:
        """Whether this saga blocks command processing."""
        return True

    def handle(self, event_book: dict) -> List[CommandBook]:
        """Process events and return commands to execute.

        Args:
            event_book: Dictionary with 'cover' and 'pages' keys

        Returns:
            List of CommandBook objects to execute
        """
        commands = []

        cover = event_book.get("cover", {})
        pages = event_book.get("pages", [])

        for page in pages:
            event = page.get("event", {})
            event_type = event.get("type_url", "")

            # Check if this is a TransactionCompleted event
            if "TransactionCompleted" not in event_type:
                continue

            # Parse the event using generated proto
            event_data = event.get("value", b"")
            transaction_completed = domains.TransactionCompleted()
            transaction_completed.ParseFromString(event_data)

            points = transaction_completed.loyalty_points_earned
            if points <= 0:
                continue

            # Get customer_id from the transaction cover
            customer_id = cover.get("root", {}).get("value", b"")
            if not customer_id:
                continue

            transaction_id = customer_id.hex() if customer_id else ""

            print(f"[{self.name}] Awarding {points} loyalty points for transaction {transaction_id[:16]}...")

            # Create AddLoyaltyPoints command using generated proto
            add_points = domains.AddLoyaltyPoints(
                points=points,
                reason=f"transaction:{transaction_id}",
            )

            command = CommandBook(
                domain="customer",
                root_id=customer_id,
                command_type="type.examples/examples.AddLoyaltyPoints",
                command_data=add_points.SerializeToString(),
            )
            commands.append(command)

        return commands


# FFI entry points for angzarr integration

_saga = LoyaltyPointsSaga()


def saga_name() -> str:
    """Return the saga name."""
    return _saga.name


def saga_domains() -> List[str]:
    """Return the domains this saga listens to."""
    return _saga.domains()


def saga_is_synchronous() -> bool:
    """Return whether this saga is synchronous."""
    return _saga.is_synchronous()


def saga_handle(event_book: dict) -> List[dict]:
    """Handle events and return commands.

    Args:
        event_book: The event book as a dictionary

    Returns:
        List of command dictionaries
    """
    commands = _saga.handle(event_book)
    return [
        {
            "domain": cmd.domain,
            "root_id": cmd.root_id,
            "command_type": cmd.command_type,
            "command_data": cmd.command_data,
        }
        for cmd in commands
    ]


if __name__ == "__main__":
    # Test the saga with a proper proto-encoded event
    transaction_completed = domains.TransactionCompleted(
        final_total_cents=10000,
        payment_method="card",
        loyalty_points_earned=100,
    )

    test_event = {
        "cover": {
            "domain": "transaction",
            "root": {"value": bytes.fromhex("0123456789abcdef0123456789abcdef")},
        },
        "pages": [
            {
                "event": {
                    "type_url": "type.examples/examples.TransactionCompleted",
                    "value": transaction_completed.SerializeToString(),
                }
            }
        ],
    }

    commands = saga_handle(test_event)
    print(f"Generated {len(commands)} command(s)")
    for cmd in commands:
        print(f"  -> {cmd['domain']}: {cmd['command_type']}")

        # Verify the command can be parsed
        add_points = domains.AddLoyaltyPoints()
        add_points.ParseFromString(cmd['command_data'])
        print(f"     points={add_points.points}, reason={add_points.reason}")
