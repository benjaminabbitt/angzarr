"""ApplyCoupon command handler."""

from datetime import datetime, timezone

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains
from .state import CartState

from .errors import CommandRejectedError, errmsg


def handle_apply_coupon(command_book, command_any, state: CartState, seq: int, log) -> angzarr.EventBook:
    if not state.exists():
        raise CommandRejectedError(errmsg.CART_NOT_FOUND)
    if not state.is_active():
        raise CommandRejectedError(errmsg.CART_CHECKED_OUT)
    if state.coupon_code:
        raise CommandRejectedError(errmsg.COUPON_ALREADY_APPLIED)

    cmd = domains.ApplyCoupon()
    command_any.Unpack(cmd)

    if not cmd.code:
        raise CommandRejectedError(errmsg.COUPON_CODE_REQUIRED)

    if cmd.coupon_type == "percentage":
        if cmd.value < 0 or cmd.value > 100:
            raise CommandRejectedError(errmsg.PERCENTAGE_RANGE)
        discount_cents = (state.subtotal_cents * cmd.value) // 100
    elif cmd.coupon_type == "fixed":
        if cmd.value < 0:
            raise CommandRejectedError(errmsg.FIXED_DISCOUNT_NEGATIVE)
        discount_cents = min(cmd.value, state.subtotal_cents)
    else:
        raise CommandRejectedError(errmsg.INVALID_COUPON_TYPE)

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
