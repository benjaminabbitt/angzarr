"""Cart business logic - command handlers."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from state import CartState


class CommandRejectedError(Exception):
    """Command was rejected due to business rule violation."""


def handle_create_cart(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if state.exists():
        raise CommandRejectedError("Cart already exists")

    cmd = domains.CreateCart()
    command_any.Unpack(cmd)

    if not cmd.customer_id:
        raise CommandRejectedError("Customer ID is required")

    log.info("creating_cart", customer_id=cmd.customer_id)

    event = domains.CartCreated(
        customer_id=cmd.customer_id,
        created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_add_item(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    cmd = domains.AddItem()
    command_any.Unpack(cmd)

    if not cmd.product_id:
        raise CommandRejectedError("Product ID is required")
    if cmd.quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")

    new_subtotal = state.subtotal_cents + (cmd.quantity * cmd.unit_price_cents)

    log.info("adding_item", product_id=cmd.product_id, quantity=cmd.quantity)

    event = domains.ItemAdded(
        product_id=cmd.product_id,
        name=cmd.name,
        quantity=cmd.quantity,
        unit_price_cents=cmd.unit_price_cents,
        new_subtotal=new_subtotal,
        added_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_update_quantity(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    cmd = domains.UpdateQuantity()
    command_any.Unpack(cmd)

    if cmd.product_id not in state.items:
        raise CommandRejectedError("Item not in cart")
    if cmd.new_quantity <= 0:
        raise CommandRejectedError("Quantity must be positive")

    item = state.items[cmd.product_id]
    old_subtotal = item.quantity * item.unit_price_cents
    new_item_subtotal = cmd.new_quantity * item.unit_price_cents
    new_subtotal = state.subtotal_cents - old_subtotal + new_item_subtotal

    log.info("updating_quantity", product_id=cmd.product_id, new_quantity=cmd.new_quantity)

    event = domains.QuantityUpdated(
        product_id=cmd.product_id,
        old_quantity=item.quantity,
        new_quantity=cmd.new_quantity,
        new_subtotal=new_subtotal,
        updated_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_remove_item(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    cmd = domains.RemoveItem()
    command_any.Unpack(cmd)

    if cmd.product_id not in state.items:
        raise CommandRejectedError("Item not in cart")

    item = state.items[cmd.product_id]
    item_subtotal = item.quantity * item.unit_price_cents
    new_subtotal = state.subtotal_cents - item_subtotal

    log.info("removing_item", product_id=cmd.product_id)

    event = domains.ItemRemoved(
        product_id=cmd.product_id,
        quantity=item.quantity,
        new_subtotal=new_subtotal,
        removed_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_apply_coupon(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")
    if state.coupon_code:
        raise CommandRejectedError("Coupon already applied")

    cmd = domains.ApplyCoupon()
    command_any.Unpack(cmd)

    if not cmd.code:
        raise CommandRejectedError("Coupon code is required")

    if cmd.coupon_type == "percentage":
        if cmd.value < 0 or cmd.value > 100:
            raise CommandRejectedError("Percentage must be 0-100")
        discount_cents = (state.subtotal_cents * cmd.value) // 100
    elif cmd.coupon_type == "fixed":
        if cmd.value < 0:
            raise CommandRejectedError("Fixed discount cannot be negative")
        discount_cents = min(cmd.value, state.subtotal_cents)
    else:
        raise CommandRejectedError("Invalid coupon type")

    log.info("applying_coupon", code=cmd.code, discount_cents=discount_cents)

    event = domains.CouponApplied(
        coupon_code=cmd.code,
        coupon_type=cmd.coupon_type,
        value=cmd.value,
        discount_cents=discount_cents,
        applied_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_clear_cart(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")

    log.info("clearing_cart")

    event = domains.CartCleared(
        new_subtotal=0,
        cleared_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )


def handle_checkout(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError("Cart does not exist")
    if not state.is_active():
        raise CommandRejectedError("Cart is already checked out")
    if not state.items:
        raise CommandRejectedError("Cart is empty")

    log.info("checking_out")

    event = domains.CartCheckedOut(
        final_subtotal=state.subtotal_cents,
        discount_cents=state.discount_cents,
        checked_out_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())),
    )

    event_any = Any()
    event_any.Pack(event, type_url_prefix="type.examples/")

    return angzarr.EventBook(
        cover=command_book.cover,
        pages=[angzarr.EventPage(num=seq, event=event_any, created_at=Timestamp(seconds=int(datetime.now(timezone.utc).timestamp())))],
    )
