"""Receipt Projector - Python Implementation.

Generates human-readable receipts when transactions complete.
"""

from typing import List, Optional, Tuple
from dataclasses import dataclass, field


@dataclass
class LineItem:
    """A line item in a transaction."""
    name: str
    quantity: int
    unit_price_cents: int


@dataclass
class TransactionState:
    """Rebuilt transaction state from events."""
    customer_id: str = ""
    items: List[LineItem] = field(default_factory=list)
    subtotal_cents: int = 0
    discount_cents: int = 0
    discount_type: str = ""
    final_total_cents: int = 0
    payment_method: str = ""
    loyalty_points_earned: int = 0
    completed: bool = False


@dataclass
class Projection:
    """A projection result."""
    projector: str
    domain: str
    root_id: bytes
    sequence: int
    projection_type: str
    projection_data: bytes


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
                parsed = self._parse_transaction_created(event_data)
                if parsed:
                    state.customer_id, state.items, state.subtotal_cents = parsed

            elif "DiscountApplied" in event_type:
                parsed = self._parse_discount_applied(event_data)
                if parsed:
                    state.discount_type, state.discount_cents = parsed

            elif "TransactionCompleted" in event_type:
                parsed = self._parse_transaction_completed(event_data)
                if parsed:
                    state.final_total_cents, state.payment_method, state.loyalty_points_earned = parsed
                    state.completed = True

        # Only generate receipt if transaction completed
        if not state.completed:
            return None

        cover = event_book.get("cover", {})
        transaction_id = cover.get("root", {}).get("value", b"").hex()

        # Generate formatted receipt text
        receipt_text = self._format_receipt(transaction_id, state)

        print(f"[{self.name}] Generated receipt for transaction {transaction_id[:16]}...")

        # Encode receipt as protobuf
        receipt_bytes = self._encode_receipt(transaction_id, state, receipt_text)

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

    def _format_receipt(self, transaction_id: str, state: TransactionState) -> str:
        """Format a human-readable receipt."""
        lines = []

        lines.append("═" * 40)
        lines.append("           RECEIPT")
        lines.append("═" * 40)
        lines.append(f"Transaction: {transaction_id[:16]}...")
        lines.append(f"Customer: {state.customer_id[:16]}..." if state.customer_id else "Customer: N/A")
        lines.append("─" * 40)

        # Items
        for item in state.items:
            line_total = item.quantity * item.unit_price_cents
            lines.append(
                f"{item.quantity} x {item.name} @ ${item.unit_price_cents / 100:.2f} = ${line_total / 100:.2f}"
            )

        lines.append("─" * 40)
        lines.append(f"Subtotal:              ${state.subtotal_cents / 100:.2f}")

        if state.discount_cents > 0:
            lines.append(f"Discount ({state.discount_type}):       -${state.discount_cents / 100:.2f}")

        lines.append("─" * 40)
        lines.append(f"TOTAL:                 ${state.final_total_cents / 100:.2f}")
        lines.append(f"Payment: {state.payment_method}")
        lines.append("─" * 40)
        lines.append(f"Loyalty Points Earned: {state.loyalty_points_earned}")
        lines.append("═" * 40)
        lines.append("     Thank you for your purchase!")
        lines.append("═" * 40)

        return "\n".join(lines)

    def _parse_transaction_created(self, data: bytes) -> Optional[Tuple[str, List[LineItem], int]]:
        """Parse TransactionCreated event."""
        customer_id = ""
        items = []
        subtotal_cents = 0

        i = 0
        while i < len(data):
            if i >= len(data):
                break
            tag = data[i]
            i += 1
            field_number = tag >> 3
            wire_type = tag & 0x07

            if field_number == 1 and wire_type == 2:
                # customer_id (string)
                length, consumed = self._decode_varint(data[i:])
                i += consumed
                if i + length <= len(data):
                    customer_id = data[i:i + length].decode("utf-8", errors="replace")
                    i += length
            elif field_number == 2 and wire_type == 2:
                # items (repeated message)
                length, consumed = self._decode_varint(data[i:])
                i += consumed
                if i + length <= len(data):
                    item = self._parse_line_item(data[i:i + length])
                    if item:
                        items.append(item)
                    i += length
            elif field_number == 3 and wire_type == 0:
                # subtotal_cents (int32)
                val, consumed = self._decode_varint(data[i:])
                i += consumed
                subtotal_cents = val
            else:
                i = self._skip_field(data, i, wire_type)

        return customer_id, items, subtotal_cents

    def _parse_line_item(self, data: bytes) -> Optional[LineItem]:
        """Parse a LineItem message."""
        name = ""
        quantity = 0
        unit_price_cents = 0

        i = 0
        while i < len(data):
            tag = data[i]
            i += 1
            field_number = tag >> 3
            wire_type = tag & 0x07

            if field_number == 2 and wire_type == 2:
                # name (string)
                length, consumed = self._decode_varint(data[i:])
                i += consumed
                if i + length <= len(data):
                    name = data[i:i + length].decode("utf-8", errors="replace")
                    i += length
            elif field_number == 3 and wire_type == 0:
                # quantity (int32)
                val, consumed = self._decode_varint(data[i:])
                i += consumed
                quantity = val
            elif field_number == 4 and wire_type == 0:
                # unit_price_cents (int32)
                val, consumed = self._decode_varint(data[i:])
                i += consumed
                unit_price_cents = val
            else:
                i = self._skip_field(data, i, wire_type)

        return LineItem(name=name, quantity=quantity, unit_price_cents=unit_price_cents)

    def _parse_discount_applied(self, data: bytes) -> Optional[Tuple[str, int]]:
        """Parse DiscountApplied event."""
        discount_type = ""
        discount_cents = 0

        i = 0
        while i < len(data):
            tag = data[i]
            i += 1
            field_number = tag >> 3
            wire_type = tag & 0x07

            if field_number == 1 and wire_type == 2:
                # discount_type (string)
                length, consumed = self._decode_varint(data[i:])
                i += consumed
                if i + length <= len(data):
                    discount_type = data[i:i + length].decode("utf-8", errors="replace")
                    i += length
            elif field_number == 3 and wire_type == 0:
                # discount_cents (int32)
                val, consumed = self._decode_varint(data[i:])
                i += consumed
                discount_cents = val
            else:
                i = self._skip_field(data, i, wire_type)

        return discount_type, discount_cents

    def _parse_transaction_completed(self, data: bytes) -> Optional[Tuple[int, str, int]]:
        """Parse TransactionCompleted event."""
        final_total_cents = 0
        payment_method = ""
        loyalty_points_earned = 0

        i = 0
        while i < len(data):
            tag = data[i]
            i += 1
            field_number = tag >> 3
            wire_type = tag & 0x07

            if field_number == 1 and wire_type == 0:
                # final_total_cents (int32)
                val, consumed = self._decode_varint(data[i:])
                i += consumed
                final_total_cents = val
            elif field_number == 2 and wire_type == 2:
                # payment_method (string)
                length, consumed = self._decode_varint(data[i:])
                i += consumed
                if i + length <= len(data):
                    payment_method = data[i:i + length].decode("utf-8", errors="replace")
                    i += length
            elif field_number == 3 and wire_type == 0:
                # loyalty_points_earned (int32)
                val, consumed = self._decode_varint(data[i:])
                i += consumed
                loyalty_points_earned = val
            else:
                i = self._skip_field(data, i, wire_type)

        return final_total_cents, payment_method, loyalty_points_earned

    def _decode_varint(self, data: bytes) -> Tuple[int, int]:
        """Decode a varint, return (value, bytes_consumed)."""
        value = 0
        shift = 0
        i = 0

        while i < len(data):
            byte = data[i]
            i += 1
            value |= (byte & 0x7F) << shift
            if byte & 0x80 == 0:
                break
            shift += 7

        return value, i

    def _skip_field(self, data: bytes, i: int, wire_type: int) -> int:
        """Skip a field based on wire type."""
        if wire_type == 0:  # Varint
            while i < len(data) and data[i] & 0x80 != 0:
                i += 1
            return i + 1
        elif wire_type == 1:  # 64-bit
            return i + 8
        elif wire_type == 2:  # Length-delimited
            length, consumed = self._decode_varint(data[i:])
            return i + consumed + length
        elif wire_type == 5:  # 32-bit
            return i + 4
        else:
            return len(data)

    def _encode_varint(self, value: int) -> bytes:
        """Encode an integer as a varint."""
        result = []
        while True:
            byte = value & 0x7F
            value >>= 7
            if value == 0:
                result.append(byte)
                break
            else:
                result.append(byte | 0x80)
        return bytes(result)

    def _encode_receipt(self, transaction_id: str, state: TransactionState, formatted_text: str) -> bytes:
        """Encode a Receipt message."""
        buf = bytearray()

        # Field 1: transaction_id (string)
        self._encode_string_field(buf, 1, transaction_id)

        # Field 2: customer_id (string)
        self._encode_string_field(buf, 2, state.customer_id)

        # Field 4: subtotal_cents (int32)
        self._encode_varint_field(buf, 4, state.subtotal_cents)

        # Field 5: discount_cents (int32)
        self._encode_varint_field(buf, 5, state.discount_cents)

        # Field 6: final_total_cents (int32)
        self._encode_varint_field(buf, 6, state.final_total_cents)

        # Field 7: payment_method (string)
        self._encode_string_field(buf, 7, state.payment_method)

        # Field 8: loyalty_points_earned (int32)
        self._encode_varint_field(buf, 8, state.loyalty_points_earned)

        # Field 10: formatted_text (string)
        self._encode_string_field(buf, 10, formatted_text)

        return bytes(buf)

    def _encode_string_field(self, buf: bytearray, field_number: int, value: str):
        """Encode a string field."""
        tag = (field_number << 3) | 2  # wire type 2 = length-delimited
        buf.append(tag)
        value_bytes = value.encode("utf-8")
        buf.extend(self._encode_varint(len(value_bytes)))
        buf.extend(value_bytes)

    def _encode_varint_field(self, buf: bytearray, field_number: int, value: int):
        """Encode a varint field."""
        tag = field_number << 3  # wire type 0 = varint
        buf.append(tag)
        buf.extend(self._encode_varint(value))


# FFI entry points for evented-rs integration

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
