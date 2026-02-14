"""Tests for Upcaster ABC and @upcasts decorator.

Tests both OO (class-based) and function-based (router) patterns.
Uses consistent domains: order, player.
Demonstrates schema evolution via event version transformation.
"""

import pytest
from google.protobuf import any_pb2

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import UpcasterRouter, upcaster
from angzarr_client.upcaster import Upcaster, upcasts

from .fixtures import (
    OrderCreated,
    OrderCreatedV1,
    PlayerRegistered,
    PlayerRegisteredV1,
)


# =============================================================================
# OO Pattern: Upcaster subclasses
# =============================================================================


class OrderUpcaster(Upcaster):
    """Upcasts order events from V1 to current version."""

    name = "upcaster-order"
    domain = "order"

    @upcasts(OrderCreatedV1, OrderCreated)
    def upcast_created(self, old: OrderCreatedV1) -> OrderCreated:
        return OrderCreated(
            order_id=old.order_id,
            customer_id=old.customer_id,
            total=0,  # New field with default
        )


class PlayerUpcaster(Upcaster):
    """Upcasts player events from V1 to current version."""

    name = "upcaster-player"
    domain = "player"

    @upcasts(PlayerRegisteredV1, PlayerRegistered)
    def upcast_registered(self, old: PlayerRegisteredV1) -> PlayerRegistered:
        return PlayerRegistered(
            player_id=old.player_id,
            display_name=old.display_name,
            registered_at="1970-01-01T00:00:00Z",  # New field with default
        )


# =============================================================================
# Function-based Pattern: UpcasterRouter with @upcaster
# =============================================================================


def build_order_upcaster_router() -> UpcasterRouter:
    """Build function-based order upcaster."""
    router = UpcasterRouter("order")

    @upcaster(OrderCreatedV1, OrderCreated)
    def upcast_created(old: OrderCreatedV1) -> OrderCreated:
        return OrderCreated(
            order_id=old.order_id,
            customer_id=old.customer_id,
            total=0,
        )

    router.on(upcast_created)
    return router


def build_player_upcaster_router() -> UpcasterRouter:
    """Build function-based player upcaster."""
    router = UpcasterRouter("player")

    @upcaster(PlayerRegisteredV1, PlayerRegistered)
    def upcast_registered(old: PlayerRegisteredV1) -> PlayerRegistered:
        return PlayerRegistered(
            player_id=old.player_id,
            display_name=old.display_name,
            registered_at="1970-01-01T00:00:00Z",
        )

    router.on(upcast_registered)
    return router


# =============================================================================
# Tests for @upcasts decorator
# =============================================================================


class TestUpcastsDecorator:
    def test_decorator_marks_handler(self):
        method = OrderUpcaster.upcast_created
        assert hasattr(method, "_is_upcaster")
        assert method._is_upcaster is True
        assert method._from_type == OrderCreatedV1
        assert method._to_type == OrderCreated

    def test_decorator_validates_missing_param(self):
        with pytest.raises(TypeError, match="must have old event parameter"):

            @upcasts(OrderCreatedV1, OrderCreated)
            def bad_method(self):
                pass

    def test_decorator_validates_missing_type_hint(self):
        with pytest.raises(TypeError, match="missing type hint"):

            @upcasts(OrderCreatedV1, OrderCreated)
            def bad_method(self, old):
                pass

    def test_decorator_validates_type_hint_mismatch(self):
        with pytest.raises(TypeError, match="doesn't match type hint"):

            @upcasts(OrderCreatedV1, OrderCreated)
            def bad_method(self, old: PlayerRegisteredV1):
                pass

    def test_decorator_validates_return_type_mismatch(self):
        with pytest.raises(TypeError, match="doesn't match return hint"):

            @upcasts(OrderCreatedV1, OrderCreated)
            def bad_method(self, old: OrderCreatedV1) -> PlayerRegistered:
                pass

    def test_decorator_preserves_function_name(self):
        method = OrderUpcaster.upcast_created
        assert method.__name__ == "upcast_created"


# =============================================================================
# Tests for Upcaster subclass validation
# =============================================================================


class TestUpcasterValidation:
    def test_missing_name_raises(self):
        with pytest.raises(TypeError, match="must define 'name'"):

            class BadUpcaster(Upcaster):
                domain = "order"

                @upcasts(OrderCreatedV1, OrderCreated)
                def handle(self, old: OrderCreatedV1) -> OrderCreated:
                    pass

    def test_missing_domain_raises(self):
        with pytest.raises(TypeError, match="must define 'domain'"):

            class BadUpcaster(Upcaster):
                name = "bad-upcaster"

                @upcasts(OrderCreatedV1, OrderCreated)
                def handle(self, old: OrderCreatedV1) -> OrderCreated:
                    pass

    def test_duplicate_handler_raises(self):
        with pytest.raises(TypeError, match="duplicate upcaster"):

            class BadUpcaster(Upcaster):
                name = "bad-upcaster"
                domain = "order"

                @upcasts(OrderCreatedV1, OrderCreated)
                def handle_one(self, old: OrderCreatedV1) -> OrderCreated:
                    pass

                @upcasts(OrderCreatedV1, OrderCreated)
                def handle_two(self, old: OrderCreatedV1) -> OrderCreated:
                    pass


# =============================================================================
# Tests for OO pattern upcasting
# =============================================================================


class TestOrderUpcasterOO:
    def test_upcasts_v1_event(self):
        upcaster = OrderUpcaster()

        old_event = OrderCreatedV1(order_id="order-1", customer_id="cust-1")
        event_any = any_pb2.Any()
        event_any.Pack(old_event)

        new_any = upcaster.upcast(event_any)

        # Verify new event
        new_event = OrderCreated()
        new_any.Unpack(new_event)
        assert new_event.order_id == "order-1"
        assert new_event.customer_id == "cust-1"
        assert new_event.total == 0

    def test_passthrough_current_version(self):
        upcaster = OrderUpcaster()

        # Current version event should pass through unchanged
        current_event = OrderCreated(
            order_id="order-1", customer_id="cust-1", total=100
        )
        event_any = any_pb2.Any()
        event_any.Pack(current_event)

        result_any = upcaster.upcast(event_any)

        # Should be same (passthrough)
        result = OrderCreated()
        result_any.Unpack(result)
        assert result.total == 100

    def test_handle_multiple_events(self):
        events = [
            types.EventPage(
                event=_pack(OrderCreatedV1(order_id="order-1", customer_id="cust-1")),
            ),
            types.EventPage(
                event=_pack(OrderCreatedV1(order_id="order-2", customer_id="cust-2")),
            ),
        ]

        result = OrderUpcaster.handle(events)

        assert len(result) == 2
        for page in result:
            event = OrderCreated()
            page.event.Unpack(event)
            assert event.total == 0  # Default added


class TestPlayerUpcasterOO:
    def test_upcasts_v1_event(self):
        upcaster = PlayerUpcaster()

        old_event = PlayerRegisteredV1(player_id="player-1", display_name="Alice")
        event_any = any_pb2.Any()
        event_any.Pack(old_event)

        new_any = upcaster.upcast(event_any)

        new_event = PlayerRegistered()
        new_any.Unpack(new_event)
        assert new_event.player_id == "player-1"
        assert new_event.display_name == "Alice"
        assert new_event.registered_at == "1970-01-01T00:00:00Z"


# =============================================================================
# Tests for function-based pattern upcasting
# =============================================================================


class TestOrderUpcasterRouter:
    def test_upcasts_v1_event(self):
        router = build_order_upcaster_router()

        events = [
            types.EventPage(
                event=_pack(OrderCreatedV1(order_id="order-1", customer_id="cust-1")),
            ),
        ]

        result = router.upcast(events)

        assert len(result) == 1
        new_event = OrderCreated()
        result[0].event.Unpack(new_event)
        assert new_event.order_id == "order-1"
        assert new_event.total == 0

    def test_passthrough_current_version(self):
        router = build_order_upcaster_router()

        events = [
            types.EventPage(
                event=_pack(
                    OrderCreated(order_id="order-1", customer_id="cust-1", total=100)
                ),
            ),
        ]

        result = router.upcast(events)

        # Should pass through unchanged
        new_event = OrderCreated()
        result[0].event.Unpack(new_event)
        assert new_event.total == 100


class TestPlayerUpcasterRouter:
    def test_upcasts_v1_event(self):
        router = build_player_upcaster_router()

        events = [
            types.EventPage(
                event=_pack(PlayerRegisteredV1(player_id="p1", display_name="Bob")),
            ),
        ]

        result = router.upcast(events)

        new_event = PlayerRegistered()
        result[0].event.Unpack(new_event)
        assert new_event.registered_at == "1970-01-01T00:00:00Z"


# =============================================================================
# Tests for OO pattern descriptor
# =============================================================================


class TestUpcasterDescriptor:
    def test_order_upcaster_descriptor(self):
        desc = OrderUpcaster.descriptor()

        assert desc.name == "upcaster-order"
        assert desc.component_type == "upcaster"
        assert desc.inputs[0].domain == "order"
        assert "OrderCreatedV1" in desc.inputs[0].types

    def test_player_upcaster_descriptor(self):
        desc = PlayerUpcaster.descriptor()

        assert desc.name == "upcaster-player"
        assert desc.inputs[0].domain == "player"
        assert "PlayerRegisteredV1" in desc.inputs[0].types


# =============================================================================
# Tests comparing both patterns produce equivalent output
# =============================================================================


class TestPatternEquivalence:
    """Verify OO and function-based patterns transform events identically."""

    def test_same_output_for_order_created_v1(self):
        # OO pattern
        oo_upcaster = OrderUpcaster()
        old_event = OrderCreatedV1(order_id="order-eq", customer_id="cust-eq")
        event_any = _pack(old_event)
        oo_result = oo_upcaster.upcast(event_any)

        # Function pattern
        fn_router = build_order_upcaster_router()
        events = [types.EventPage(event=_pack(old_event))]
        fn_result = fn_router.upcast(events)

        # Both produce identical output
        oo_event = OrderCreated()
        oo_result.Unpack(oo_event)

        fn_event = OrderCreated()
        fn_result[0].event.Unpack(fn_event)

        assert oo_event.order_id == fn_event.order_id == "order-eq"
        assert oo_event.customer_id == fn_event.customer_id == "cust-eq"
        assert oo_event.total == fn_event.total == 0


# =============================================================================
# Helper functions
# =============================================================================


def _pack(msg) -> any_pb2.Any:
    """Pack a message into Any."""
    result = any_pb2.Any()
    result.Pack(msg)
    return result
