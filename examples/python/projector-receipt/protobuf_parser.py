"""Protobuf wire format parsing utilities."""

from typing import List, Optional, Tuple

from models import LineItem


def decode_varint(data: bytes) -> Tuple[int, int]:
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


def skip_field(data: bytes, i: int, wire_type: int) -> int:
    """Skip a field based on wire type."""
    if wire_type == 0:  # Varint
        while i < len(data) and data[i] & 0x80 != 0:
            i += 1
        return i + 1
    elif wire_type == 1:  # 64-bit
        return i + 8
    elif wire_type == 2:  # Length-delimited
        length, consumed = decode_varint(data[i:])
        return i + consumed + length
    elif wire_type == 5:  # 32-bit
        return i + 4
    else:
        return len(data)


def parse_line_item(data: bytes) -> Optional[LineItem]:
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
            length, consumed = decode_varint(data[i:])
            i += consumed
            if i + length <= len(data):
                name = data[i:i + length].decode("utf-8", errors="replace")
                i += length
        elif field_number == 3 and wire_type == 0:
            # quantity (int32)
            val, consumed = decode_varint(data[i:])
            i += consumed
            quantity = val
        elif field_number == 4 and wire_type == 0:
            # unit_price_cents (int32)
            val, consumed = decode_varint(data[i:])
            i += consumed
            unit_price_cents = val
        else:
            i = skip_field(data, i, wire_type)

    return LineItem(name=name, quantity=quantity, unit_price_cents=unit_price_cents)


def parse_transaction_created(data: bytes) -> Optional[Tuple[str, List[LineItem], int]]:
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
            length, consumed = decode_varint(data[i:])
            i += consumed
            if i + length <= len(data):
                customer_id = data[i:i + length].decode("utf-8", errors="replace")
                i += length
        elif field_number == 2 and wire_type == 2:
            # items (repeated message)
            length, consumed = decode_varint(data[i:])
            i += consumed
            if i + length <= len(data):
                item = parse_line_item(data[i:i + length])
                if item:
                    items.append(item)
                i += length
        elif field_number == 3 and wire_type == 0:
            # subtotal_cents (int32)
            val, consumed = decode_varint(data[i:])
            i += consumed
            subtotal_cents = val
        else:
            i = skip_field(data, i, wire_type)

    return customer_id, items, subtotal_cents


def parse_discount_applied(data: bytes) -> Optional[Tuple[str, int]]:
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
            length, consumed = decode_varint(data[i:])
            i += consumed
            if i + length <= len(data):
                discount_type = data[i:i + length].decode("utf-8", errors="replace")
                i += length
        elif field_number == 3 and wire_type == 0:
            # discount_cents (int32)
            val, consumed = decode_varint(data[i:])
            i += consumed
            discount_cents = val
        else:
            i = skip_field(data, i, wire_type)

    return discount_type, discount_cents


def parse_transaction_completed(data: bytes) -> Optional[Tuple[int, str, int]]:
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
            val, consumed = decode_varint(data[i:])
            i += consumed
            final_total_cents = val
        elif field_number == 2 and wire_type == 2:
            # payment_method (string)
            length, consumed = decode_varint(data[i:])
            i += consumed
            if i + length <= len(data):
                payment_method = data[i:i + length].decode("utf-8", errors="replace")
                i += length
        elif field_number == 3 and wire_type == 0:
            # loyalty_points_earned (int32)
            val, consumed = decode_varint(data[i:])
            i += consumed
            loyalty_points_earned = val
        else:
            i = skip_field(data, i, wire_type)

    return final_total_cents, payment_method, loyalty_points_earned
