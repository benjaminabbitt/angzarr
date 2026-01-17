"""Receipt Projector - Python Implementation.

Generates human-readable receipts when transactions complete.
"""

from typing import List, Optional

from models import Projection, TransactionState
from protobuf_parser import (
    parse_transaction_created,
    parse_discount_applied,
    parse_transaction_completed,
)
from protobuf_encoder import encode_receipt
from receipt_formatter import format_receipt


class ReceiptProjector:
    """Projector that generates receipts when transactions complete."""

    def __init__(self):
        self.name = "receipt"

    def domains(self) -> List[str]:
        """Return the domains this projector listens to."""
        return ["transaction"]

    def is_synchronous(self) -> bool:
        """Whether this projector blocks command processing."""
        return True

    def project(self, event_book: dict) -> Optional[Projection]:
        """Process events and generate a receipt if transaction completed.

        Args:
            event_book: Dictionary with 'cover' and 'pages' keys

        Returns:
            Projection if receipt generated, None otherwise
        """
        # Rebuild transaction state from all events
        state = TransactionState()

        pages = event_book.get("pages", [])
        for page in pages:
            event = page.get("event", {})
            event_type = event.get("type_url", "")
            event_data = event.get("value", b"")

            if "TransactionCreated" in event_type:
                parsed = parse_transaction_created(event_data)
                if parsed:
                    state.customer_id, state.items, state.subtotal_cents = parsed

            elif "DiscountApplied" in event_type:
                parsed = parse_discount_applied(event_data)
                if parsed:
                    state.discount_type, state.discount_cents = parsed

            elif "TransactionCompleted" in event_type:
                parsed = parse_transaction_completed(event_data)
                if parsed:
                    state.final_total_cents, state.payment_method, state.loyalty_points_earned = parsed
                    state.completed = True

        # Only generate receipt if transaction completed
        if not state.completed:
            return None

        cover = event_book.get("cover", {})
        transaction_id = cover.get("root", {}).get("value", b"").hex()

        # Generate formatted receipt text
        receipt_text = format_receipt(transaction_id, state)

        print(f"[{self.name}] Generated receipt for transaction {transaction_id[:16]}...")

        # Encode receipt as protobuf
        receipt_bytes = encode_receipt(transaction_id, state, receipt_text)

        # Get sequence from last page
        sequence = 0
        if pages:
            last_page = pages[-1]
            sequence = last_page.get("sequence", {}).get("num", 0)

        return Projection(
            projector=self.name,
            domain=cover.get("domain", "transaction"),
            root_id=cover.get("root", {}).get("value", b""),
            sequence=sequence,
            projection_type="type.examples/examples.Receipt",
            projection_data=receipt_bytes,
        )


# FFI entry points for angzarr integration

_projector = ReceiptProjector()


def projector_name() -> str:
    """Return the projector name."""
    return _projector.name


def projector_domains() -> List[str]:
    """Return the domains this projector listens to."""
    return _projector.domains()


def projector_is_synchronous() -> bool:
    """Return whether this projector is synchronous."""
    return _projector.is_synchronous()


def projector_project(event_book: dict) -> Optional[dict]:
    """Project events and return a projection.

    Args:
        event_book: The event book as a dictionary

    Returns:
        Projection dictionary or None
    """
    projection = _projector.project(event_book)
    if projection is None:
        return None

    return {
        "projector": projection.projector,
        "domain": projection.domain,
        "root_id": projection.root_id,
        "sequence": projection.sequence,
        "projection_type": projection.projection_type,
        "projection_data": projection.projection_data,
    }


if __name__ == "__main__":
    # Test the projector
    test_event = {
        "cover": {
            "domain": "transaction",
            "root": {"value": bytes.fromhex("0123456789abcdef0123456789abcdef")},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.TransactionCreated",
                    "value": bytes([
                        # customer_id = "cust123"
                        0x0A, 0x07, 0x63, 0x75, 0x73, 0x74, 0x31, 0x32, 0x33,
                        # item: Widget, qty 2, $9.99
                        0x12, 0x0C, 0x12, 0x06, 0x57, 0x69, 0x64, 0x67, 0x65, 0x74, 0x18, 0x02, 0x20, 0xE7, 0x07,
                        # subtotal_cents = 1998
                        0x18, 0xCE, 0x0F,
                    ]),
                },
            },
            {
                "sequence": {"num": 1},
                "event": {
                    "type_url": "type.examples/examples.TransactionCompleted",
                    "value": bytes([
                        0x08, 0xCE, 0x0F,  # field 1: 1998 (final_total_cents)
                        0x12, 0x04, 0x63, 0x61, 0x72, 0x64,  # field 2: "card"
                        0x18, 0x13,  # field 3: 19 (loyalty_points_earned)
                    ]),
                },
            },
        ],
    }

    from proto import domains_pb2 as domains

    projection = projector_project(test_event)
    if projection:
        print(f"Generated projection: {projection['projector']}")
        receipt = domains.Receipt()
        receipt.ParseFromString(projection["projection_data"])
        print("\nReceipt Preview:")
        print("-" * 40)
        print(receipt.formatted_text)
    else:
        print("No projection generated")
