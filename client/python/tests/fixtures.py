"""Shared test fixtures for consistent domain messages across all component tests.

Domains:
- order: Order lifecycle (CreateOrder → OrderCreated → OrderCompleted)
- inventory: Stock management (ReserveStock → StockReserved)
- fulfillment: Shipping (CreateShipment → ShipmentCreated)
- player: Player management (PlayerRegistered, ScoreUpdated)
"""

from dataclasses import dataclass, field, fields


# =============================================================================
# Base class for test proto messages
# =============================================================================


class TestProto:
    """Base class for test protobuf-like messages.

    Provides automatic serialization using | as field separator.
    Subclasses just define fields as dataclass.
    """

    DESCRIPTOR = None  # Set by subclass or metaclass

    def SerializeToString(self, deterministic=None) -> bytes:
        values = [str(getattr(self, f.name)) for f in fields(self)]
        return "|".join(values).encode()

    def ParseFromString(self, data: bytes) -> None:
        parts = data.decode().split("|")
        for i, f in enumerate(fields(self)):
            if i < len(parts):
                value = parts[i]
                if f.type == int:
                    setattr(self, f.name, int(value) if value else 0)
                else:
                    setattr(self, f.name, value)


def _make_descriptor(full_name: str):
    """Create a DESCRIPTOR with full_name attribute."""
    return type("Descriptor", (), {"full_name": full_name})()


# =============================================================================
# Order domain - Commands
# =============================================================================


@dataclass
class CreateOrder(TestProto):
    """Command to create an order."""

    DESCRIPTOR = _make_descriptor("order.CreateOrder")
    order_id: str = ""
    customer_id: str = ""
    items: list = field(default_factory=list)


@dataclass
class CompleteOrder(TestProto):
    """Command to complete an order."""

    DESCRIPTOR = _make_descriptor("order.CompleteOrder")
    order_id: str = ""


# =============================================================================
# Order domain - Events
# =============================================================================


@dataclass
class OrderCreatedV1(TestProto):
    """Event: order was created (old version without total)."""

    DESCRIPTOR = _make_descriptor("order.OrderCreatedV1")
    order_id: str = ""
    customer_id: str = ""


@dataclass
class OrderCreated(TestProto):
    """Event: order was created."""

    DESCRIPTOR = _make_descriptor("order.OrderCreated")
    order_id: str = ""
    customer_id: str = ""
    total: int = 0


@dataclass
class OrderCompleted(TestProto):
    """Event: order was completed."""

    DESCRIPTOR = _make_descriptor("order.OrderCompleted")
    order_id: str = ""
    shipped_at: str = ""


# =============================================================================
# Inventory domain - Commands
# =============================================================================


@dataclass
class ReserveStock(TestProto):
    """Command to reserve stock."""

    DESCRIPTOR = _make_descriptor("inventory.ReserveStock")
    order_id: str = ""
    sku: str = ""
    quantity: int = 0


# =============================================================================
# Inventory domain - Events
# =============================================================================


@dataclass
class StockReserved(TestProto):
    """Event: stock was reserved."""

    DESCRIPTOR = _make_descriptor("inventory.StockReserved")
    order_id: str = ""
    sku: str = ""
    quantity: int = 0


@dataclass
class StockUpdated(TestProto):
    """Event: stock level was updated."""

    DESCRIPTOR = _make_descriptor("inventory.StockUpdated")
    sku: str = ""
    quantity: int = 0


# =============================================================================
# Fulfillment domain - Commands
# =============================================================================


@dataclass
class CreateShipment(TestProto):
    """Command to create a shipment."""

    DESCRIPTOR = _make_descriptor("fulfillment.CreateShipment")
    order_id: str = ""
    address: str = ""


# =============================================================================
# Fulfillment domain - Events
# =============================================================================


@dataclass
class ShipmentCreated(TestProto):
    """Event: shipment was created."""

    DESCRIPTOR = _make_descriptor("fulfillment.ShipmentCreated")
    order_id: str = ""
    tracking_number: str = ""


# =============================================================================
# Player domain - Events (for projector tests)
# =============================================================================


@dataclass
class PlayerRegisteredV1(TestProto):
    """Event: player was registered (old version without registered_at)."""

    DESCRIPTOR = _make_descriptor("player.PlayerRegisteredV1")
    player_id: str = ""
    display_name: str = ""


@dataclass
class PlayerRegistered(TestProto):
    """Event: player was registered."""

    DESCRIPTOR = _make_descriptor("player.PlayerRegistered")
    player_id: str = ""
    display_name: str = ""
    registered_at: str = ""


@dataclass
class ScoreUpdated(TestProto):
    """Event: player score was updated."""

    DESCRIPTOR = _make_descriptor("player.ScoreUpdated")
    player_id: str = ""
    game_id: str = ""
    score_delta: int = 0
    new_total: int = 0
