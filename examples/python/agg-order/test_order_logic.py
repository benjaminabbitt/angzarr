"""Tests for order command handlers via CommandRouter."""

import pytest
from google.protobuf.any_pb2 import Any as AnyProto

from angzarr import types_pb2 as types
from errors import CommandRejectedError
from proto import domains_pb2 as domains
from main import router


def _pack_command(command, domain: str = "order") -> types.ContextualCommand:
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
    command, prior_events: types.EventBook, domain: str = "order",
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


def _create_order_events(
    customer_id: str = "cust-1",
    subtotal_cents: int = 5000,
) -> types.EventBook:
    """Create an EventBook with an OrderCreated event."""
    event = domains.OrderCreated(
        customer_id=customer_id,
        subtotal_cents=subtotal_cents,
        items=[
            domains.LineItem(
                product_id="prod-1",
                name="Widget",
                quantity=2,
                unit_price_cents=2500,
            ),
        ],
    )
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    return types.EventBook(
        pages=[types.EventPage(num=0, event=event_any)],
    )


def _apply_discount_events(
    prior: types.EventBook, points_used: int = 100, discount_cents: int = 500,
) -> types.EventBook:
    """Append a LoyaltyDiscountApplied event to existing events."""
    event = domains.LoyaltyDiscountApplied(
        points_used=points_used,
        discount_cents=discount_cents,
    )
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    pages = list(prior.pages) + [types.EventPage(num=len(prior.pages), event=event_any)]
    return types.EventBook(pages=pages)


def _submit_payment_events(
    prior: types.EventBook, payment_method: str = "credit_card", amount_cents: int = 4500,
) -> types.EventBook:
    """Append a PaymentSubmitted event to existing events."""
    event = domains.PaymentSubmitted(
        payment_method=payment_method,
        amount_cents=amount_cents,
    )
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    pages = list(prior.pages) + [types.EventPage(num=len(prior.pages), event=event_any)]
    return types.EventBook(pages=pages)


def _complete_order_events(
    prior: types.EventBook, payment_reference: str = "ref-123",
) -> types.EventBook:
    """Append an OrderCompleted event to existing events."""
    event = domains.OrderCompleted(
        payment_reference=payment_reference,
        payment_method="credit_card",
        final_total_cents=4500,
        loyalty_points_earned=45,
    )
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    pages = list(prior.pages) + [types.EventPage(num=len(prior.pages), event=event_any)]
    return types.EventBook(pages=pages)


def _cancel_order_events(
    prior: types.EventBook, reason: str = "changed mind",
) -> types.EventBook:
    """Append an OrderCancelled event to existing events."""
    event = domains.OrderCancelled(reason=reason, loyalty_points_used=0)
    event_any = AnyProto()
    event_any.Pack(event, type_url_prefix="type.examples/")
    pages = list(prior.pages) + [types.EventPage(num=len(prior.pages), event=event_any)]
    return types.EventBook(pages=pages)


class TestCreateOrder:
    def test_create_order_success(self):
        cmd = domains.CreateOrder(
            customer_id="cust-1",
            items=[
                domains.LineItem(
                    product_id="prod-1",
                    name="Widget",
                    quantity=2,
                    unit_price_cents=2500,
                ),
            ],
        )
        resp = router.dispatch(_pack_command(cmd))

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1
        assert resp.events.pages[0].event.type_url.endswith("OrderCreated")

        event = domains.OrderCreated()
        resp.events.pages[0].event.Unpack(event)
        assert event.customer_id == "cust-1"
        assert event.subtotal_cents == 5000

    def test_create_order_already_exists(self):
        prior = _create_order_events()
        cmd = domains.CreateOrder(
            customer_id="cust-2",
            items=[domains.LineItem(product_id="p", name="x", quantity=1, unit_price_cents=100)],
        )
        with pytest.raises(CommandRejectedError, match="already exists"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_create_order_missing_customer_id(self):
        cmd = domains.CreateOrder(
            customer_id="",
            items=[domains.LineItem(product_id="p", name="x", quantity=1, unit_price_cents=100)],
        )
        with pytest.raises(CommandRejectedError, match="Customer ID is required"):
            router.dispatch(_pack_command(cmd))

    def test_create_order_no_items(self):
        cmd = domains.CreateOrder(customer_id="cust-1", items=[])
        with pytest.raises(CommandRejectedError, match="at least one item"):
            router.dispatch(_pack_command(cmd))


class TestApplyLoyaltyDiscount:
    def test_apply_discount_success(self):
        prior = _create_order_events()
        cmd = domains.ApplyLoyaltyDiscount(points=100, discount_cents=500)
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.LoyaltyDiscountApplied()
        resp.events.pages[0].event.Unpack(event)
        assert event.points_used == 100
        assert event.discount_cents == 500

    def test_apply_discount_order_does_not_exist(self):
        cmd = domains.ApplyLoyaltyDiscount(points=100, discount_cents=500)
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_apply_discount_order_not_pending(self):
        prior = _create_order_events()
        prior = _apply_discount_events(prior)
        prior = _submit_payment_events(prior)
        cmd = domains.ApplyLoyaltyDiscount(points=50, discount_cents=250)
        with pytest.raises(CommandRejectedError, match="not in pending state"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_apply_discount_already_applied(self):
        prior = _create_order_events()
        prior = _apply_discount_events(prior)
        cmd = domains.ApplyLoyaltyDiscount(points=50, discount_cents=250)
        with pytest.raises(CommandRejectedError, match="already applied"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_apply_discount_points_must_be_positive(self):
        prior = _create_order_events()
        cmd = domains.ApplyLoyaltyDiscount(points=0, discount_cents=500)
        with pytest.raises(CommandRejectedError, match="Points must be positive"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_apply_discount_must_be_positive(self):
        prior = _create_order_events()
        cmd = domains.ApplyLoyaltyDiscount(points=100, discount_cents=0)
        with pytest.raises(CommandRejectedError, match="Discount must be positive"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_apply_discount_cannot_exceed_subtotal(self):
        prior = _create_order_events(subtotal_cents=5000)
        cmd = domains.ApplyLoyaltyDiscount(points=100, discount_cents=6000)
        with pytest.raises(CommandRejectedError, match="cannot exceed subtotal"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestSubmitPayment:
    def test_submit_payment_success(self):
        prior = _create_order_events()
        cmd = domains.SubmitPayment(payment_method="credit_card", amount_cents=5000)
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.PaymentSubmitted()
        resp.events.pages[0].event.Unpack(event)
        assert event.payment_method == "credit_card"
        assert event.amount_cents == 5000

    def test_submit_payment_with_discount(self):
        prior = _create_order_events()
        prior = _apply_discount_events(prior, discount_cents=500)
        cmd = domains.SubmitPayment(payment_method="credit_card", amount_cents=4500)
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"

    def test_submit_payment_order_does_not_exist(self):
        cmd = domains.SubmitPayment(payment_method="credit_card", amount_cents=5000)
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_submit_payment_order_not_pending(self):
        prior = _create_order_events()
        prior = _submit_payment_events(prior, amount_cents=5000)
        cmd = domains.SubmitPayment(payment_method="credit_card", amount_cents=5000)
        with pytest.raises(CommandRejectedError, match="not in pending state"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_submit_payment_missing_method(self):
        prior = _create_order_events()
        cmd = domains.SubmitPayment(payment_method="", amount_cents=5000)
        with pytest.raises(CommandRejectedError, match="Payment method is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_submit_payment_amount_mismatch(self):
        prior = _create_order_events()
        cmd = domains.SubmitPayment(payment_method="credit_card", amount_cents=9999)
        with pytest.raises(CommandRejectedError, match="must match order total"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestConfirmPayment:
    def test_confirm_payment_success(self):
        prior = _create_order_events()
        prior = _submit_payment_events(prior, amount_cents=5000)
        cmd = domains.ConfirmPayment(payment_reference="ref-123")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.OrderCompleted()
        resp.events.pages[0].event.Unpack(event)
        assert event.payment_reference == "ref-123"
        assert event.final_total_cents == 5000
        assert event.loyalty_points_earned == 50

    def test_confirm_payment_with_discount(self):
        prior = _create_order_events()
        prior = _apply_discount_events(prior, discount_cents=500)
        prior = _submit_payment_events(prior, amount_cents=4500)
        cmd = domains.ConfirmPayment(payment_reference="ref-456")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        event = domains.OrderCompleted()
        resp.events.pages[0].event.Unpack(event)
        assert event.final_total_cents == 4500
        assert event.loyalty_points_earned == 45

    def test_confirm_payment_order_does_not_exist(self):
        cmd = domains.ConfirmPayment(payment_reference="ref-123")
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_confirm_payment_not_submitted(self):
        prior = _create_order_events()
        cmd = domains.ConfirmPayment(payment_reference="ref-123")
        with pytest.raises(CommandRejectedError, match="Payment not submitted"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_confirm_payment_missing_reference(self):
        prior = _create_order_events()
        prior = _submit_payment_events(prior, amount_cents=5000)
        cmd = domains.ConfirmPayment(payment_reference="")
        with pytest.raises(CommandRejectedError, match="Payment reference is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestCancelOrder:
    def test_cancel_order_success(self):
        prior = _create_order_events()
        cmd = domains.CancelOrder(reason="changed mind")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        assert resp.WhichOneof("result") == "events"
        event = domains.OrderCancelled()
        resp.events.pages[0].event.Unpack(event)
        assert event.reason == "changed mind"

    def test_cancel_order_with_loyalty_points(self):
        prior = _create_order_events()
        prior = _apply_discount_events(prior, points_used=100, discount_cents=500)
        cmd = domains.CancelOrder(reason="changed mind")
        resp = router.dispatch(_pack_command_with_events(cmd, prior))

        event = domains.OrderCancelled()
        resp.events.pages[0].event.Unpack(event)
        assert event.loyalty_points_used == 100

    def test_cancel_order_does_not_exist(self):
        cmd = domains.CancelOrder(reason="no order")
        with pytest.raises(CommandRejectedError, match="does not exist"):
            router.dispatch(_pack_command(cmd))

    def test_cancel_completed_order(self):
        prior = _create_order_events()
        prior = _submit_payment_events(prior, amount_cents=5000)
        prior = _complete_order_events(prior)
        cmd = domains.CancelOrder(reason="too late")
        with pytest.raises(CommandRejectedError, match="Cannot cancel completed order"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_cancel_already_cancelled(self):
        prior = _create_order_events()
        prior = _cancel_order_events(prior)
        cmd = domains.CancelOrder(reason="again")
        with pytest.raises(CommandRejectedError, match="already cancelled"):
            router.dispatch(_pack_command_with_events(cmd, prior))

    def test_cancel_order_missing_reason(self):
        prior = _create_order_events()
        cmd = domains.CancelOrder(reason="")
        with pytest.raises(CommandRejectedError, match="reason is required"):
            router.dispatch(_pack_command_with_events(cmd, prior))


class TestUnknownCommand:
    def test_unknown_command_raises_value_error(self):
        unknown = AnyProto(type_url="type.examples/UnknownCommand", value=b"")
        ctx = types.ContextualCommand(
            command=types.CommandBook(
                cover=types.Cover(domain="order"),
                pages=[types.CommandPage(sequence=0, command=unknown)],
            ),
        )
        with pytest.raises(ValueError, match="Unknown command type"):
            router.dispatch(ctx)
