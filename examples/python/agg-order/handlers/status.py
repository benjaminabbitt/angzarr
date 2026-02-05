"""Order status enum.

Provides typed status values instead of string literals.
"""

from enum import StrEnum


class OrderStatus(StrEnum):
    """Order aggregate status values."""

    PENDING = "pending"
    PAYMENT_SUBMITTED = "payment_submitted"
    COMPLETED = "completed"
    CANCELLED = "cancelled"
