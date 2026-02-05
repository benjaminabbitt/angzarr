"""Fulfillment status enum.

Provides typed status values instead of string literals.
"""

from enum import StrEnum


class FulfillmentStatus(StrEnum):
    """Fulfillment aggregate status values."""

    PENDING = "pending"
    PICKING = "picking"
    PACKING = "packing"
    SHIPPED = "shipped"
    DELIVERED = "delivered"
