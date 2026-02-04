"""Tests for inventory reservation saga event handlers."""

import uuid

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from identity import inventory_product_root, to_proto_bytes
from proto import cart_pb2 as cart
from proto import inventory_pb2 as inventory

from main import (
    TARGET_DOMAIN,
    handle_cart_cleared,
    handle_item_added,
    handle_item_removed,
    handle_quantity_updated,
    router,
)


ROOT_BYTES = uuid.uuid4().bytes
CORRELATION_ID = "test-correlation-123"


def _make_root() -> types.UUID:
    return types.UUID(value=ROOT_BYTES)


def _pack_event(msg) -> AnyProto:
    event_any = AnyProto()
    event_any.Pack(msg, type_url_prefix="type.examples/")
    return event_any


def _make_event_book(event_type: str, event_msg) -> types.EventBook:
    """Create an EventBook with a single event for router dispatch testing."""
    event_any = _pack_event(event_msg)
    return types.EventBook(
        cover=types.Cover(
            domain="cart",
            root=_make_root(),
            correlation_id=CORRELATION_ID,
        ),
        pages=[types.EventPage(num=0, event=event_any)],
    )


class TestItemAdded:
    def test_item_added_generates_reserve_stock(self):
        added = cart.ItemAdded(
            product_id="SKU-001",
            name="Widget",
            quantity=5,
            unit_price_cents=1000,
            new_subtotal=5000,
        )
        event_any = _pack_event(added)
        root = _make_root()

        commands = handle_item_added(event_any, root, CORRELATION_ID)

        assert len(commands) == 1
        cmd_book = commands[0]
        assert cmd_book.cover.domain == TARGET_DOMAIN
        assert cmd_book.cover.correlation_id == CORRELATION_ID

        reserve = inventory.ReserveStock()
        cmd_book.pages[0].command.Unpack(reserve)
        assert reserve.quantity == 5
        assert reserve.order_id == ROOT_BYTES.hex()

        expected_root = to_proto_bytes(inventory_product_root("SKU-001"))
        assert cmd_book.cover.root.value == expected_root


class TestItemRemoved:
    def test_item_removed_generates_release_reservation(self):
        removed = cart.ItemRemoved(
            product_id="SKU-001",
            quantity=5,
            new_subtotal=0,
        )
        event_any = _pack_event(removed)
        root = _make_root()

        commands = handle_item_removed(event_any, root, CORRELATION_ID)

        assert len(commands) == 1
        cmd_book = commands[0]
        assert cmd_book.cover.domain == TARGET_DOMAIN

        release = inventory.ReleaseReservation()
        cmd_book.pages[0].command.Unpack(release)
        assert release.order_id == ROOT_BYTES.hex()

        expected_root = to_proto_bytes(inventory_product_root("SKU-001"))
        assert cmd_book.cover.root.value == expected_root


class TestQuantityUpdated:
    def test_quantity_updated_generates_release_and_reserve(self):
        updated = cart.QuantityUpdated(
            product_id="SKU-001",
            old_quantity=3,
            new_quantity=7,
            new_subtotal=7000,
        )
        event_any = _pack_event(updated)
        root = _make_root()

        commands = handle_quantity_updated(event_any, root, CORRELATION_ID)

        assert len(commands) == 2

        # First: ReleaseReservation
        release = inventory.ReleaseReservation()
        commands[0].pages[0].command.Unpack(release)
        assert release.order_id == ROOT_BYTES.hex()
        assert commands[0].pages[0].command.type_url.endswith("ReleaseReservation")

        # Second: ReserveStock with new quantity
        reserve = inventory.ReserveStock()
        commands[1].pages[0].command.Unpack(reserve)
        assert reserve.quantity == 7
        assert reserve.order_id == ROOT_BYTES.hex()
        assert commands[1].pages[0].command.type_url.endswith("ReserveStock")

        # Both target the same product root
        expected_root = to_proto_bytes(inventory_product_root("SKU-001"))
        assert commands[0].cover.root.value == expected_root
        assert commands[1].cover.root.value == expected_root


class TestCartCleared:
    def test_cart_cleared_releases_all_items(self):
        cleared = cart.CartCleared(
            new_subtotal=0,
            items=[
                cart.CartItem(
                    product_id="SKU-001",
                    name="Widget",
                    quantity=2,
                    unit_price_cents=1000,
                ),
                cart.CartItem(
                    product_id="SKU-002",
                    name="Gadget",
                    quantity=3,
                    unit_price_cents=2000,
                ),
            ],
        )
        event_any = _pack_event(cleared)
        root = _make_root()

        commands = handle_cart_cleared(event_any, root, CORRELATION_ID)

        assert len(commands) == 2

        for cmd_book in commands:
            assert cmd_book.cover.domain == TARGET_DOMAIN
            release = inventory.ReleaseReservation()
            cmd_book.pages[0].command.Unpack(release)
            assert release.order_id == ROOT_BYTES.hex()

        # Each targets the correct product root
        assert commands[0].cover.root.value == to_proto_bytes(
            inventory_product_root("SKU-001")
        )
        assert commands[1].cover.root.value == to_proto_bytes(
            inventory_product_root("SKU-002")
        )


class TestDeterministicProductRoot:
    def test_deterministic_product_root(self):
        root1 = inventory_product_root("SKU-001")
        root2 = inventory_product_root("SKU-001")
        root3 = inventory_product_root("SKU-002")

        assert root1 == root2
        assert root1 != root3


class TestRouterDispatch:
    def test_ignores_unrelated_events(self):
        event_any = AnyProto(
            type_url="type.examples/examples.SomeOtherEvent", value=b"\x01\x02\x03"
        )
        book = types.EventBook(
            cover=types.Cover(
                domain="cart",
                root=_make_root(),
                correlation_id=CORRELATION_ID,
            ),
            pages=[types.EventPage(num=0, event=event_any)],
        )
        commands = router.dispatch(book)
        assert commands == []

    def test_nil_root_returns_empty(self):
        added = cart.ItemAdded(
            product_id="SKU-001",
            name="Widget",
            quantity=1,
            unit_price_cents=100,
            new_subtotal=100,
        )
        event_any = _pack_event(added)

        commands = handle_item_added(event_any, None, CORRELATION_ID)
        assert commands == []

    def test_router_dispatches_item_added(self):
        added = cart.ItemAdded(
            product_id="SKU-001",
            name="Widget",
            quantity=3,
            unit_price_cents=500,
            new_subtotal=1500,
        )
        book = _make_event_book("ItemAdded", added)
        commands = router.dispatch(book)

        assert len(commands) == 1
        assert commands[0].cover.domain == TARGET_DOMAIN
        assert commands[0].pages[0].command.type_url.endswith("ReserveStock")

    def test_router_dispatches_cart_cleared(self):
        cleared = cart.CartCleared(
            new_subtotal=0,
            items=[
                cart.CartItem(product_id="SKU-A", name="A", quantity=1, unit_price_cents=100),
            ],
        )
        book = _make_event_book("CartCleared", cleared)
        commands = router.dispatch(book)

        assert len(commands) == 1
        assert commands[0].pages[0].command.type_url.endswith("ReleaseReservation")
