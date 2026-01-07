"""Transaction Log Projector - Pretty prints transaction events to terminal."""

import sys
from pathlib import Path
from typing import Optional

# Add common to path
sys.path.insert(0, str(Path(__file__).parent.parent / "common"))

from event_logger import project_events


class TransactionLogProjector:
    """Projector that logs transaction events."""

    def __init__(self):
        self.name = "log-transaction"

    def domains(self) -> list[str]:
        """Return the domains this projector listens to."""
        return ["transaction"]

    def is_synchronous(self) -> bool:
        """Whether this projector blocks command processing."""
        return False

    def project(self, event_book: dict) -> None:
        """Process events and print them to terminal."""
        project_events(event_book)


_projector = TransactionLogProjector()


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

    print("Testing Transaction Log Projector")
    print("=" * 60)

    ts = Timestamp()
    ts.FromDatetime(datetime.now())

    tx_created = domains.TransactionCreated(
        customer_id="abcdef0123456789abcdef0123456789",
        items=[
            domains.LineItem(
                product_id="prod1",
                name="Widget",
                quantity=2,
                unit_price_cents=999,
            ),
        ],
        subtotal_cents=1998,
        created_at=ts,
    )
    tx_event = {
        "cover": {
            "domain": "transaction",
            "root": {"value": bytes.fromhex("0123456789abcdef0123456789abcdef")},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.TransactionCreated",
                    "value": tx_created.SerializeToString(),
                },
            }
        ],
    }
    projector_project(tx_event)

    tx_completed = domains.TransactionCompleted(
        final_total_cents=1998,
        payment_method="card",
        loyalty_points_earned=19,
        completed_at=ts,
    )
    completed_event = {
        "cover": {
            "domain": "transaction",
            "root": {"value": bytes.fromhex("0123456789abcdef0123456789abcdef")},
        },
        "pages": [
            {
                "sequence": {"num": 1},
                "event": {
                    "type_url": "type.examples/examples.TransactionCompleted",
                    "value": tx_completed.SerializeToString(),
                },
            }
        ],
    }
    projector_project(completed_event)

    print()
    print("=" * 60)
    print("Done!")
