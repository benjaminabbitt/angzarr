"""Data models for the Receipt Projector."""

from typing import List
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
