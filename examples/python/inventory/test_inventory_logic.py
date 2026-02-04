"""Tests for inventory command handlers via CommandRouter."""

import pytest
from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import domains_pb2 as domains
from main import router


def _pack_command(command, domain: str = "inventory") -> types.ContextualCommand:
    """Pack a domain command into a ContextualCommand."""
    command_any = AnyProto()
    command_any.Pack(command, type_url_prefix="type.examples/")

    return types.ContextualCommand(
        command=types.CommandBook(
            cover=types.Cover(domain=domain),
            pages=[types.CommandPage(sequence=0, command=command_any)],
        ),
    )


def _pack_command_with_events(
    command, prior_events: types.EventBook, domain: str = "inventory",
) -> types.ContextualCommand:
    """Pack a domain command with prior events."""
    command_any = AnyProto()
    command_any.Pack(command, type_url_prefix="type.examples/")

    return types.ContextualCommand(
        command=types.CommandBook(
            cover=types.Cover(domain=domain),
            pages=[types.CommandPage(sequence=0, command=command_any)],
        ),
        events=prior_events,
    )


def _initialize_stock_events(
    product_id: str = "PROD-1", quantity: int = 100, low_stock_threshold: int = 10,
) -> types.EventBook:
    """Create an EventBook with a StockInitialized event."""
    event = domains.StockInitialized(
        product_id=product_id, quantity=quantity, low_stock_threshold=low_stock_threshold,
    )
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    return types.EventBook(
        pages=[types.EventPage(num=0, event=event_any)],
    )


def _receive_stock_events(
    prior: types.EventBook, quantity: int, new_on_hand: int, reference: str = "PO-001",
) -> types.EventBook:
    """Append a StockReceived event to existing events."""
    event = domains.StockReceived(
        quantity=quantity, new_on_hand=new_on_hand, reference=reference,
    )
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    pages = list(prior.pages) + [types.EventPage(num=len(prior.pages), event=event_any)]
    return types.EventBook(pages=pages)


def _reserve_stock_events(
    prior: types.EventBook, quantity: int, order_id: str, new_available: int,
) -> types.EventBook:
    """Append a StockReserved event to existing events."""
    event = domains.StockReserved(
        quantity=quantity, order_id=order_id, new_available=new_available,
    )
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    pages = list(prior.pages) + [types.EventPage(num=len(prior.pages), event=event_any)]
    return types.EventBook(pages=pages)


class TestInitializeStock:
    def test_initialize_stock_success(self):
        cmd = domains.InitializeStock(
            product_id="PROD-1", quantity=100, low_stock_threshold=10,
        )
        resp = router.dispatch(_pack_command(cmd))

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1
        assert resp.events.pages[0].event.type_url.endswith("StockInitialized")

        event = domains.StockInitialized()
        resp.events.pages[0].event.Unpack(event)
        assert event.product_id == "PROD-1"
        assert event.quantity == 100
        assert event.low_stock_threshold == 10

    def test_initialize_stock_already_initialized(self):
        prior = _initialize_stock_events()
        cmd = domains.InitializeStock(
            product_id="PROD-2", quantity=50, low_stock_threshold=5,
        )
        with pytest.raises(CommandRejectedError, match="already initialized"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_initialize_stock_missing_product_id(self):
        cmd = domains.InitializeStock(product_id="", quantity=100, low_stock_threshold=10)
        with pytest.raises(CommandRejectedError, match="Product ID is required"):
            router.dispatch(_pack_command(cmd))

    def test_initialize_stock_negative_quantity(self):
        cmd = domains.InitializeStock(
            product_id="PROD-1", quantity=-1, low_stock_threshold=10,
        )
        with pytest.raises(CommandRejectedError, match="Quantity cannot be negative"):
            router.dispatch(_pack_command(cmd))

    def test_initialize_stock_negative_threshold(self):
        cmd = domains.InitializeStock(
            product_id="PROD-1", quantity=100, low_stock_threshold=-1,
        )
        with pytest.raises(CommandRejectedError, match="threshold cannot be negative"):
            router.dispatch(_pack_command(cmd))


class TestReceiveStock:
    def test_receive_stock_success(self):
        prior = _initialize_stock_events()
        cmd = domains.ReceiveStock(quantity=50, reference="PO-001")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.StockReceived()
        resp.events.pages[0].event.Unpack(event)
        assert event.quantity == 50
        assert event.new_on_hand == 150
        assert event.reference == "PO-001"

    def test_receive_stock_requires_initialization(self):
        cmd = domains.ReceiveStock(quantity=50, reference="PO-001")
        with pytest.raises(CommandRejectedError, match="not initialized"):
            router.dispatch(_pack_command(cmd))

    def test_receive_stock_must_be_positive(self):
        prior = _initialize_stock_events()
        cmd = domains.ReceiveStock(quantity=0, reference="PO-001")
        with pytest.raises(CommandRejectedError, match="must be positive"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestReserveStock:
    def test_reserve_stock_success(self):
        prior = _initialize_stock_events()
        cmd = domains.ReserveStock(quantity=10, order_id="ORD-1")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.StockReserved()
        resp.events.pages[0].event.Unpack(event)
        assert event.quantity == 10
        assert event.order_id == "ORD-1"
        assert event.new_available == 90

    def test_reserve_stock_requires_initialization(self):
        cmd = domains.ReserveStock(quantity=10, order_id="ORD-1")
        with pytest.raises(CommandRejectedError, match="not initialized"):
            router.dispatch(_pack_command(cmd))

    def test_reserve_stock_must_be_positive(self):
        prior = _initialize_stock_events()
        cmd = domains.ReserveStock(quantity=0, order_id="ORD-1")
        with pytest.raises(CommandRejectedError, match="must be positive"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_reserve_stock_missing_order_id(self):
        prior = _initialize_stock_events()
        cmd = domains.ReserveStock(quantity=10, order_id="")
        with pytest.raises(CommandRejectedError, match="Order ID is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_reserve_stock_duplicate_reservation(self):
        prior = _initialize_stock_events()
        prior = _reserve_stock_events(prior, quantity=10, order_id="ORD-1", new_available=90)
        cmd = domains.ReserveStock(quantity=5, order_id="ORD-1")
        with pytest.raises(CommandRejectedError, match="already exists"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_reserve_stock_insufficient_stock(self):
        prior = _initialize_stock_events(quantity=10)
        cmd = domains.ReserveStock(quantity=20, order_id="ORD-1")
        with pytest.raises(CommandRejectedError, match="Insufficient stock"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_reserve_stock_triggers_low_stock_alert(self):
        prior = _initialize_stock_events(quantity=20, low_stock_threshold=15)
        cmd = domains.ReserveStock(quantity=10, order_id="ORD-1")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 2
        assert resp.events.pages[0].event.type_url.endswith("StockReserved")
        assert resp.events.pages[1].event.type_url.endswith("LowStockAlert")

        alert = domains.LowStockAlert()
        resp.events.pages[1].event.Unpack(alert)
        assert alert.available == 10
        assert alert.threshold == 15


class TestReleaseReservation:
    def test_release_reservation_success(self):
        prior = _initialize_stock_events()
        prior = _reserve_stock_events(prior, quantity=10, order_id="ORD-1", new_available=90)
        cmd = domains.ReleaseReservation(order_id="ORD-1")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.ReservationReleased()
        resp.events.pages[0].event.Unpack(event)
        assert event.order_id == "ORD-1"
        assert event.quantity == 10
        assert event.new_available == 100

    def test_release_reservation_requires_initialization(self):
        cmd = domains.ReleaseReservation(order_id="ORD-1")
        with pytest.raises(CommandRejectedError, match="not initialized"):
            router.dispatch(_pack_command(cmd))

    def test_release_reservation_missing_order_id(self):
        prior = _initialize_stock_events()
        cmd = domains.ReleaseReservation(order_id="")
        with pytest.raises(CommandRejectedError, match="Order ID is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_release_reservation_not_found(self):
        prior = _initialize_stock_events()
        cmd = domains.ReleaseReservation(order_id="ORD-NONE")
        with pytest.raises(CommandRejectedError, match="No reservation found"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestCommitReservation:
    def test_commit_reservation_success(self):
        prior = _initialize_stock_events()
        prior = _reserve_stock_events(prior, quantity=10, order_id="ORD-1", new_available=90)
        cmd = domains.CommitReservation(order_id="ORD-1")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.ReservationCommitted()
        resp.events.pages[0].event.Unpack(event)
        assert event.order_id == "ORD-1"
        assert event.quantity == 10
        assert event.new_on_hand == 90

    def test_commit_reservation_requires_initialization(self):
        cmd = domains.CommitReservation(order_id="ORD-1")
        with pytest.raises(CommandRejectedError, match="not initialized"):
            router.dispatch(_pack_command(cmd))

    def test_commit_reservation_missing_order_id(self):
        prior = _initialize_stock_events()
        cmd = domains.CommitReservation(order_id="")
        with pytest.raises(CommandRejectedError, match="Order ID is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_commit_reservation_not_found(self):
        prior = _initialize_stock_events()
        cmd = domains.CommitReservation(order_id="ORD-NONE")
        with pytest.raises(CommandRejectedError, match="No reservation found"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestUnknownCommand:
    def test_unknown_command_raises_value_error(self):
        unknown = AnyProto(type_url="type.examples/UnknownCommand", value=b"")
        ctx = types.ContextualCommand(
            command=types.CommandBook(
                cover=types.Cover(domain="inventory"),
                pages=[types.CommandPage(sequence=0, command=unknown)],
            ),
        )
        with pytest.raises(ValueError, match="Unknown command type"):
            router.dispatch(ctx)
