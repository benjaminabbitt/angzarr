"""Cucumber BDD tests for Receipt Projector using pytest-bdd."""

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

import sys
sys.path.insert(0, '.')

from proto import domains_pb2 as domains
from receipt_projector import ReceiptProjector, LineItem


# Load scenarios from feature file
scenarios('projector-receipt.feature')


class ReceiptProjectorTestContext:
    """Test context for receipt projector BDD scenarios."""

    def __init__(self):
        self.projector = ReceiptProjector()
        self.pages = []
        self.customer_id = ""
        self.subtotal_cents = 0
        self.projection = None
        self.receipt = None
        self.error = None


@pytest.fixture
def ctx():
    """Fixture providing fresh test context for each scenario."""
    return ReceiptProjectorTestContext()


def _build_event_book(ctx):
    """Build event book from accumulated pages."""
    return {
        "cover": {
            "domain": "transaction",
            "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08\x09\x0a\x0b\x0c\x0d\x0e\x0f\x10"},
        },
        "pages": ctx.pages,
    }


def _datatable_to_line_items(datatable):
    """Convert pytest-bdd datatable (list of lists) to LineItem objects.

    Args:
        datatable: List of lists where row 0 is headers, rows 1+ are data.

    Returns:
        List of LineItem protobuf objects and the subtotal.
    """
    if not datatable or len(datatable) < 2:
        return [], 0

    headers = datatable[0]
    items = []
    subtotal = 0
    for row in datatable[1:]:
        row_dict = dict(zip(headers, row))
        item = domains.LineItem(
            product_id=row_dict['product_id'],
            name=row_dict['name'],
            quantity=int(row_dict['quantity']),
            unit_price_cents=int(row_dict['unit_price_cents']),
        )
        items.append(item)
        subtotal += int(row_dict['quantity']) * int(row_dict['unit_price_cents'])
    return items, subtotal


# --- Given steps ---

@given(parsers.parse('a TransactionCreated event with customer "{customer_id}" and subtotal {subtotal:d}'))
def transaction_created_simple(ctx, customer_id, subtotal):
    ctx.customer_id = customer_id
    ctx.subtotal_cents = subtotal
    event = domains.TransactionCreated(
        customer_id=customer_id,
        subtotal_cents=subtotal,
    )
    ctx.pages.append({
        "sequence": {"num": len(ctx.pages)},
        "event": {
            "type_url": "type.examples/examples.TransactionCreated",
            "value": event.SerializeToString(),
        },
    })


@given(parsers.parse('a TransactionCreated event with customer "{customer_id}" and items:'))
def transaction_created_with_items(ctx, customer_id, datatable):
    ctx.customer_id = customer_id
    items, subtotal = _datatable_to_line_items(datatable)

    ctx.subtotal_cents = subtotal
    event = domains.TransactionCreated(
        customer_id=customer_id,
        items=items,
        subtotal_cents=subtotal,
    )
    ctx.pages.append({
        "sequence": {"num": len(ctx.pages)},
        "event": {
            "type_url": "type.examples/examples.TransactionCreated",
            "value": event.SerializeToString(),
        },
    })


@given(parsers.parse('a DiscountApplied event with {discount:d} cents discount'))
def discount_applied_event(ctx, discount):
    event = domains.DiscountApplied(
        discount_type="coupon",
        discount_cents=discount,
    )
    ctx.pages.append({
        "sequence": {"num": len(ctx.pages)},
        "event": {
            "type_url": "type.examples/examples.DiscountApplied",
            "value": event.SerializeToString(),
        },
    })


@given(parsers.parse('a TransactionCompleted event with total {total:d} and payment "{payment}"'))
def transaction_completed_simple(ctx, total, payment):
    event = domains.TransactionCompleted(
        final_total_cents=total,
        payment_method=payment,
    )
    ctx.pages.append({
        "sequence": {"num": len(ctx.pages)},
        "event": {
            "type_url": "type.examples/examples.TransactionCompleted",
            "value": event.SerializeToString(),
        },
    })


@given(parsers.parse('a TransactionCompleted event with total {total:d} and payment "{payment}" earning {points:d} points'))
def transaction_completed_with_points(ctx, total, payment, points):
    event = domains.TransactionCompleted(
        final_total_cents=total,
        payment_method=payment,
        loyalty_points_earned=points,
    )
    ctx.pages.append({
        "sequence": {"num": len(ctx.pages)},
        "event": {
            "type_url": "type.examples/examples.TransactionCompleted",
            "value": event.SerializeToString(),
        },
    })


# --- When steps ---

@when('I project the events')
def project_events(ctx):
    event_book = _build_event_book(ctx)
    try:
        ctx.projection = ctx.projector.project(event_book)
        if ctx.projection:
            # Decode the receipt from the projection
            ctx.receipt = domains.Receipt()
            ctx.receipt.ParseFromString(ctx.projection.projection_data)
        ctx.error = None
    except Exception as e:
        ctx.error = e
        ctx.projection = None
        ctx.receipt = None


# --- Then steps ---

@then('no projection is generated')
def no_projection_generated(ctx):
    assert ctx.error is None, f"Expected no error but got: {ctx.error}"
    assert ctx.projection is None, "Expected no projection but got one"


@then('a Receipt projection is generated')
def receipt_projection_generated(ctx):
    assert ctx.error is None, f"Expected success but got error: {ctx.error}"
    assert ctx.projection is not None, "Expected projection but got None"
    assert ctx.receipt is not None, "Expected receipt but got None"


@then(parsers.parse('the receipt has customer_id "{customer_id}"'))
def receipt_has_customer_id(ctx, customer_id):
    assert ctx.receipt is not None, "No receipt to check"
    assert ctx.receipt.customer_id == customer_id, \
        f"Expected customer_id '{customer_id}', got '{ctx.receipt.customer_id}'"


@then(parsers.parse('the receipt has final_total_cents {total:d}'))
def receipt_has_final_total(ctx, total):
    assert ctx.receipt is not None, "No receipt to check"
    assert ctx.receipt.final_total_cents == total, \
        f"Expected final_total_cents {total}, got {ctx.receipt.final_total_cents}"


@then(parsers.parse('the receipt has payment_method "{payment}"'))
def receipt_has_payment_method(ctx, payment):
    assert ctx.receipt is not None, "No receipt to check"
    assert ctx.receipt.payment_method == payment, \
        f"Expected payment_method '{payment}', got '{ctx.receipt.payment_method}'"


@then(parsers.parse('the receipt has subtotal_cents {subtotal:d}'))
def receipt_has_subtotal(ctx, subtotal):
    assert ctx.receipt is not None, "No receipt to check"
    assert ctx.receipt.subtotal_cents == subtotal, \
        f"Expected subtotal_cents {subtotal}, got {ctx.receipt.subtotal_cents}"


@then(parsers.parse('the receipt has discount_cents {discount:d}'))
def receipt_has_discount(ctx, discount):
    assert ctx.receipt is not None, "No receipt to check"
    assert ctx.receipt.discount_cents == discount, \
        f"Expected discount_cents {discount}, got {ctx.receipt.discount_cents}"


@then(parsers.parse('the receipt has loyalty_points_earned {points:d}'))
def receipt_has_loyalty_points(ctx, points):
    assert ctx.receipt is not None, "No receipt to check"
    assert ctx.receipt.loyalty_points_earned == points, \
        f"Expected loyalty_points_earned {points}, got {ctx.receipt.loyalty_points_earned}"


@then(parsers.parse('the receipt formatted_text contains "{text}"'))
def receipt_formatted_text_contains(ctx, text):
    assert ctx.receipt is not None, "No receipt to check"
    assert text in ctx.receipt.formatted_text, \
        f"Expected formatted_text to contain '{text}', got:\n{ctx.receipt.formatted_text}"
