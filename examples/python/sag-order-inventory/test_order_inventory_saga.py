"""Tests for order-inventory saga event handlers."""

import uuid

from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from identity import inventory_product_root, to_proto_bytes
from proto import order_pb2 as order
from proto import inventory_pb2 as inventory

from main import TARGET_DOMAIN, handle_order_created, router


ROOT_BYTES = uuid.uuid4().bytes
CORRELATION_ID = "test-correlation-123"


def _make_root() -> types.UUID:
    return types.UUID(value=ROOT_BYTES)


def _pack_event(msg) -> AnyProto:
    event_any = AnyProto()
    event_any.Pack(msg, type_url_prefix="type.examples/")
    return event_any


def _make_event_book(event_msg) -> types.EventBook:
    """Create an EventBook with a single event for router dispatch testing."""
    event_any = _pack_event(event_msg)
    return types.EventBook(
        cover=types.Cover(
            domain="order",
            root=_make_root(),
            correlation_id=CORRELATION_ID,
        ),
        pages=[types.EventPage(num=0, event=event_any)],
    )


class TestOrderCreated:
    def test_order_created_generates_reserve_stock_per_item(self):
        created = order.OrderCreated(
            customer_id="cust-001",
            items=[
                order.LineItem(product_id="SKU-001", quantity=2, unit_price_cents=1000),
                order.LineItem(product_id="SKU-002", quantity=3, unit_price_cents=2000),
            ],
            subtotal_cents=8000,
        )
        event_any = _pack_event(created)
        root = _make_root()

        commands = handle_order_created(event_any, root, CORRELATION_ID)

        assert len(commands) == 2

        # First item
        cmd1 = commands[0]
        assert cmd1.cover.domain == TARGET_DOMAIN
        assert cmd1.cover.correlation_id == CORRELATION_ID
        reserve1 = inventory.ReserveStock()
        cmd1.pages[0].command.Unpack(reserve1)
        assert reserve1.quantity == 2
        assert reserve1.order_id == ROOT_BYTES.hex()
        expected_root1 = to_proto_bytes(inventory_product_root("SKU-001"))
        assert cmd1.cover.root.value == expected_root1

        # Second item
        cmd2 = commands[1]
        reserve2 = inventory.ReserveStock()
        cmd2.pages[0].command.Unpack(reserve2)
        assert reserve2.quantity == 3
        expected_root2 = to_proto_bytes(inventory_product_root("SKU-002"))
        assert cmd2.cover.root.value == expected_root2

    def test_nil_root_returns_empty(self):
        created = order.OrderCreated(
            customer_id="cust-001",
            items=[order.LineItem(product_id="SKU-001", quantity=1, unit_price_cents=100)],
            subtotal_cents=100,
        )
        event_any = _pack_event(created)

        commands = handle_order_created(event_any, None, CORRELATION_ID)
        assert commands == []

    def test_no_items_returns_empty(self):
        created = order.OrderCreated(customer_id="cust-001", items=[], subtotal_cents=0)
        event_any = _pack_event(created)
        root = _make_root()

        commands = handle_order_created(event_any, root, CORRELATION_ID)
        assert commands == []


class TestRouterDispatch:
    def test_router_dispatches_order_created(self):
        created = order.OrderCreated(
            customer_id="cust-001",
            items=[order.LineItem(product_id="SKU-001", quantity=5, unit_price_cents=500)],
            subtotal_cents=2500,
        )
        book = _make_event_book(created)
        commands = router.dispatch(book)

        assert len(commands) == 1
        assert commands[0].cover.domain == TARGET_DOMAIN
        assert commands[0].pages[0].command.type_url.endswith("ReserveStock")

    def test_ignores_unrelated_events(self):
        event_any = AnyProto(
            type_url="type.examples/examples.SomeOtherEvent", value=b"\x01\x02\x03"
        )
        book = types.EventBook(
            cover=types.Cover(
                domain="order",
                root=_make_root(),
                correlation_id=CORRELATION_ID,
            ),
            pages=[types.EventPage(num=0, event=event_any)],
        )
        commands = router.dispatch(book)
        assert commands == []


class TestDeterministicProductRoot:
    def test_deterministic_product_root(self):
        root1 = inventory_product_root("SKU-001")
        root2 = inventory_product_root("SKU-001")
        root3 = inventory_product_root("SKU-002")

        assert root1 == root2
        assert root1 != root3
