"""Protobuf wire format encoding utilities."""

from models import TransactionState


def encode_varint(value: int) -> bytes:
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


def encode_string_field(buf: bytearray, field_number: int, value: str) -> None:
    """Encode a string field."""
    tag = (field_number << 3) | 2  # wire type 2 = length-delimited
    buf.append(tag)
    value_bytes = value.encode("utf-8")
    buf.extend(encode_varint(len(value_bytes)))
    buf.extend(value_bytes)


def encode_varint_field(buf: bytearray, field_number: int, value: int) -> None:
    """Encode a varint field."""
    tag = field_number << 3  # wire type 0 = varint
    buf.append(tag)
    buf.extend(encode_varint(value))


def encode_receipt(transaction_id: str, state: TransactionState, formatted_text: str) -> bytes:
    """Encode a Receipt message."""
    buf = bytearray()

    # Field 1: transaction_id (string)
    encode_string_field(buf, 1, transaction_id)

    # Field 2: customer_id (string)
    encode_string_field(buf, 2, state.customer_id)

    # Field 4: subtotal_cents (int32)
    encode_varint_field(buf, 4, state.subtotal_cents)

    # Field 5: discount_cents (int32)
    encode_varint_field(buf, 5, state.discount_cents)

    # Field 6: final_total_cents (int32)
    encode_varint_field(buf, 6, state.final_total_cents)

    # Field 7: payment_method (string)
    encode_string_field(buf, 7, state.payment_method)

    # Field 8: loyalty_points_earned (int32)
    encode_varint_field(buf, 8, state.loyalty_points_earned)

    # Field 10: formatted_text (string)
    encode_string_field(buf, 10, formatted_text)

    return bytes(buf)
