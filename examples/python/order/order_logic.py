"""Order command handlers and business logic."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from state import OrderState


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


def handle_create_order(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Order already exists")

    cmd = domains.CreateOrder()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("Customer ID is required")
    if not cmd.items:
        raise CommandRejectedError("Order must have at least one item")

    subtotal = sum(item.quantity * item.unit_price_cents for item in cmd.items)

    log.info("creating_order", customer_id=cmd.customer_id, item_count=len(cmd.items))

    event = domains.OrderCreated(
        customer_id=cmd.customer_id,
        subtotal_cents=subtotal,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )
    event.items.extend(cmd.items)

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_apply_loyalty_discount(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_pending():
        raise CommandRejectedError("Order is not in pending state")
    if state.loyalty_points_used > 0:
        raise CommandRejectedError("Loyalty discount already applied")

    cmd = domains.ApplyLoyaltyDiscount()
    command_any.Unpack(cmd)

    if cmd.points <= 0:
        raise CommandRejectedError("Points must be positive")
    if cmd.discount_cents <= 0:
        raise CommandRejectedError("Discount must be positive")
    if cmd.discount_cents > state.subtotal_cents:
        raise CommandRejectedError("Discount cannot exceed subtotal")

    log.info("applying_loyalty_discount", points=cmd.points, discount_cents=cmd.discount_cents)

    event = domains.LoyaltyDiscountApplied(
        points_used=cmd.points,
        discount_cents=cmd.discount_cents,
        applied_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_submit_payment(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_pending():
        raise CommandRejectedError("Order is not in pending state")

    cmd = domains.SubmitPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_method:
        raise CommandRejectedError("Payment method is required")
    expected_total = state.total_after_discount()
    if cmd.amount_cents != expected_total:
        raise CommandRejectedError("Payment amount must match order total")

    log.info("submitting_payment", method=cmd.payment_method, amount=cmd.amount_cents)

    event = domains.PaymentSubmitted(
        payment_method=cmd.payment_method,
        amount_cents=cmd.amount_cents,
        submitted_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_confirm_payment(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if not state.is_payment_submitted():
        raise CommandRejectedError("Payment not submitted")

    cmd = domains.ConfirmPayment()
    command_any.Unpack(cmd)

    if not cmd.payment_reference:
        raise CommandRejectedError("Payment reference is required")

    # 1 point per dollar
    loyalty_points_earned = state.total_after_discount() // 100

    log.info("confirming_payment", reference=cmd.payment_reference)

    event = domains.OrderCompleted(
        final_total_cents=state.total_after_discount(),
        payment_method=state.payment_method,
        payment_reference=cmd.payment_reference,
        loyalty_points_earned=loyalty_points_earned,
        completed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_cancel_order(command_book, command_any, state: OrderState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Order does not exist")
    if state.is_completed():
        raise CommandRejectedError("Cannot cancel completed order")
    if state.is_cancelled():
        raise CommandRejectedError("Order already cancelled")

    cmd = domains.CancelOrder()
    command_any.Unpack(cmd)

    if not cmd.reason:
        raise CommandRejectedError("Cancellation reason is required")

    log.info("cancelling_order", reason=cmd.reason)

    event = domains.OrderCancelled(
        reason=cmd.reason,
        cancelled_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
        loyalty_points_used=state.loyalty_points_used,
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
