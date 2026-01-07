"""Customer Log Projector - Pretty prints customer events to terminal."""

import sys
from pathlib import Path
from typing import Optional

# Add common to path
sys.path.insert(0, str(Path(__file__).parent.parent / "common"))

from event_logger import project_events


class CustomerLogProjector:
    """Projector that logs customer events."""

    def __init__(self):
        self.name = "log-customer"

    def domains(self) -> list[str]:
        """Return the domains this projector listens to."""
        return ["customer"]

    def is_synchronous(self) -> bool:
        """Whether this projector blocks command processing."""
        return False

    def project(self, event_book: dict) -> None:
        """Process events and print them to terminal."""
        project_events(event_book)


_projector = CustomerLogProjector()


def projector_name() -> str:
    """Return the projector name."""
    return _projector.name


def projector_domains() -> list[str]:
    """Return the domains this projector listens to."""
    return _projector.domains()


def projector_is_synchronous() -> bool:
    """Return whether this projector is synchronous."""
    return _projector.is_synchronous()


def projector_project(event_book: dict) -> Optional[dict]:
    """Project events - logs them and returns None."""
    _projector.project(event_book)
    return None


if __name__ == "__main__":
    from datetime import datetime
    from proto import domains_pb2 as domains
    from google.protobuf.timestamp_pb2 import Timestamp

    print("Testing Customer Log Projector")
    print("=" * 60)

    ts = Timestamp()
    ts.FromDatetime(datetime.now())

    customer_created = domains.CustomerCreated(
        name="John Doe",
        email="john@example.com",
        created_at=ts,
    )
    customer_event = {
        "cover": {
            "domain": "customer",
            "root": {"value": bytes.fromhex("abcdef0123456789abcdef0123456789")},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.CustomerCreated",
                    "value": customer_created.SerializeToString(),
                },
            }
        ],
    }
    projector_project(customer_event)

    points_added = domains.LoyaltyPointsAdded(
        points=100,
        new_balance=100,
        reason="welcome_bonus",
    )
    points_event = {
        "cover": {
            "domain": "customer",
            "root": {"value": bytes.fromhex("abcdef0123456789abcdef0123456789")},
        },
        "pages": [
            {
                "sequence": {"num": 1},
                "event": {
                    "type_url": "type.examples/examples.LoyaltyPointsAdded",
                    "value": points_added.SerializeToString(),
                },
            }
        ],
    }
    projector_project(points_event)

    print()
    print("=" * 60)
    print("Done!")
