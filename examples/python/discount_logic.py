"""Order Discount Calculator - Python Business Logic.

Demonstrates discount calculation logic that can be tested in-process
via PyO3 or deployed as a gRPC service.

Business Rules:
1. Percentage discounts require at least one item in the order
2. Only one percentage discount can be active at a time
3. Coupons can stack with percentage discounts
4. Bulk discounts apply when 5+ items are in the order
"""

from evented import (
    business_logic,
    CommandContext,
    EventBook,
)
from evented.business_logic import create_event, handle


def get_command_type(type_url: str) -> str:
    """Extract command type from type URL."""
    return type_url.split("/")[-1] if "/" in type_url else type_url.split(".")[-1]


def get_event_type(event) -> str:
    """Extract event type from an EventPage."""
    if not event or not event.Event:
        return ""
    type_url = event.Event.type_url
    return type_url.split("/")[-1] if "/" in type_url else type_url.split(".")[-1]


def has_order(ctx: CommandContext) -> bool:
    """Check if order exists (has OrderCreated event)."""
    return any(get_event_type(e) == "OrderCreated" for e in ctx.prior_events)


def has_items(ctx: CommandContext) -> bool:
    """Check if order has any items."""
    return any(get_event_type(e) == "ItemAdded" for e in ctx.prior_events)


def has_active_discount(ctx: CommandContext) -> bool:
    """Check if order already has an active percentage discount."""
    discount_count = sum(
        1 for e in ctx.prior_events if get_event_type(e) == "DiscountApplied"
    )
    removed_count = sum(
        1 for e in ctx.prior_events if get_event_type(e) == "DiscountRemoved"
    )
    return discount_count > removed_count


def count_items(ctx: CommandContext) -> int:
    """Count the number of items in the order."""
    return sum(1 for e in ctx.prior_events if get_event_type(e) == "ItemAdded")


@business_logic(domain="discounts")
def handle_discounts(ctx: CommandContext) -> EventBook:
    """Handle discount domain commands."""
    command_type = get_command_type(ctx.command.type_url)
    next_seq = len(ctx.prior_events)

    if command_type == "ApplyPercentageDiscount":
        return apply_percentage_discount(ctx, next_seq)
    elif command_type == "ApplyCoupon":
        return apply_coupon(ctx, next_seq)
    elif command_type == "RemoveDiscount":
        return remove_discount(ctx, next_seq)
    elif command_type == "CalculateBulkDiscount":
        return calculate_bulk_discount(ctx, next_seq)
    else:
        raise ValueError(f"Unknown discount command: {command_type}")


def apply_percentage_discount(ctx: CommandContext, seq: int) -> EventBook:
    """Apply a percentage discount to the order.

    Rules:
    - Order must exist
    - Order must have items
    - No existing percentage discount
    """
    if not has_order(ctx):
        raise ValueError("Cannot apply discount: no order exists")

    if not has_items(ctx):
        raise ValueError("Cannot apply discount: order has no items")

    if has_active_discount(ctx):
        raise ValueError("Cannot apply discount: order already has a discount")

    return create_event(
        domain=ctx.domain,
        root_id=ctx.root_id,
        sequence=seq,
        event_type="discounts.DiscountApplied",
        event_data=ctx.command.value,
    )


def apply_coupon(ctx: CommandContext, seq: int) -> EventBook:
    """Apply a coupon code to the order.

    Rules:
    - Order must exist
    - Coupons can stack with other discounts
    """
    if not has_order(ctx):
        raise ValueError("Cannot apply coupon: no order exists")

    return create_event(
        domain=ctx.domain,
        root_id=ctx.root_id,
        sequence=seq,
        event_type="discounts.CouponApplied",
        event_data=ctx.command.value,
    )


def remove_discount(ctx: CommandContext, seq: int) -> EventBook:
    """Remove a discount from the order.

    Rules:
    - Order must have an active discount
    """
    if not has_active_discount(ctx):
        raise ValueError("Cannot remove discount: no active discount")

    return create_event(
        domain=ctx.domain,
        root_id=ctx.root_id,
        sequence=seq,
        event_type="discounts.DiscountRemoved",
        event_data=ctx.command.value,
    )


def calculate_bulk_discount(ctx: CommandContext, seq: int) -> EventBook:
    """Calculate bulk discount for large orders.

    Rules:
    - Requires 5+ items for bulk discount
    """
    if not has_order(ctx):
        raise ValueError("Cannot calculate bulk discount: no order exists")

    item_count = count_items(ctx)
    if item_count < 5:
        raise ValueError(f"Cannot apply bulk discount: need 5+ items, have {item_count}")

    return create_event(
        domain=ctx.domain,
        root_id=ctx.root_id,
        sequence=seq,
        event_type="discounts.BulkDiscountCalculated",
        event_data=ctx.command.value,
    )
