"""Cucumber BDD tests for Transaction business logic using pytest-bdd."""

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from google.protobuf.any_pb2 import Any

# Import generated proto (assumes proto is in the path)
import sys
sys.path.insert(0, '.')
from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from transaction_logic import TransactionLogic

# Load scenarios from feature file
scenarios('transaction.feature')


class TransactionTestContext:
    """Test context for transaction BDD scenarios."""

    def __init__(self):
        self.logic = TransactionLogic()
        self.prior_events = []
        self.result_event = None
        self.error = None
        self.state = None


@pytest.fixture
def ctx():
    """Fixture providing fresh test context for each scenario."""
    return TransactionTestContext()


# --- Given steps ---

@given('no prior events for the aggregate')
def no_prior_events(ctx):
    ctx.prior_events = []


@given(parsers.parse('a TransactionCreated event with customer "{customer_id}" and subtotal {subtotal:d}'))
def transaction_created_event(ctx, customer_id, subtotal):
    event = domains.TransactionCreated(
        customer_id=customer_id,
        subtotal_cents=subtotal,
    )
    event_any = Any()
    event_any.Pack(event)
    ctx.prior_events.append(event_any)


@given('a TransactionCompleted event')
def transaction_completed_event(ctx):
    event = domains.TransactionCompleted(
        final_total_cents=0,
        payment_method="card",
        loyalty_points_earned=0,
    )
    event_any = Any()
    event_any.Pack(event)
    ctx.prior_events.append(event_any)


@given(parsers.parse('a DiscountApplied event with {discount_cents:d} cents discount'))
def discount_applied_event(ctx, discount_cents):
    event = domains.DiscountApplied(
        discount_type="fixed",
        value=discount_cents,
        discount_cents=discount_cents,
    )
    event_any = Any()
    event_any.Pack(event)
    ctx.prior_events.append(event_any)


# --- Helper to build event book ---

def build_event_book(prior_events):
    if not prior_events:
        return None
    pages = [angzarr.EventPage(event=event) for event in prior_events]
    return angzarr.EventBook(pages=pages)


# --- Helper to convert datatable to LineItems ---

def datatable_to_line_items(datatable):
    """Convert pytest-bdd datatable (list of lists) to LineItem objects.

    Args:
        datatable: List of lists where row 0 is headers, rows 1+ are data.

    Returns:
        List of LineItem protobuf objects.
    """
    if not datatable or len(datatable) < 2:
        return []

    headers = datatable[0]
    items = []
    for row in datatable[1:]:
        row_dict = dict(zip(headers, row))
        item = domains.LineItem(
            product_id=row_dict['product_id'],
            name=row_dict['name'],
            quantity=int(row_dict['quantity']),
            unit_price_cents=int(row_dict['unit_price_cents']),
        )
        items.append(item)
    return items


# --- When steps ---

@when(parsers.re(r'I handle a CreateTransaction command with customer "(?P<customer_id>[^"]*)" and items:'))
def handle_create_transaction_with_items(ctx, customer_id, datatable):
    """Handle CreateTransaction with items from datatable.

    The datatable parameter is reserved by pytest-bdd and automatically
    receives the Gherkin table as a list of lists.
    """
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        line_items = datatable_to_line_items(datatable)

        cmd = domains.CreateTransaction(
            customer_id=customer_id,
            items=line_items,
        )
        cmd_any = Any()
        cmd_any.Pack(cmd)

        cmd_book = angzarr.CommandBook(
            pages=[angzarr.CommandPage(command=cmd_any)]
        )

        contextual = angzarr.ContextualCommand(
            command=cmd_book,
            events=event_book
        )

        result = ctx.logic.handle(contextual)
        if result.pages:
            result_any = result.pages[0].event
            if result_any.type_url.endswith('TransactionCreated'):
                ctx.result_event = domains.TransactionCreated()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when(parsers.parse('I handle a CreateTransaction command with customer "{customer_id}" and no items'))
def handle_create_transaction_no_items(ctx, customer_id):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        cmd = domains.CreateTransaction(
            customer_id=customer_id,
            items=[],
        )
        cmd_any = Any()
        cmd_any.Pack(cmd)

        cmd_book = angzarr.CommandBook(
            pages=[angzarr.CommandPage(command=cmd_any)]
        )

        contextual = angzarr.ContextualCommand(
            command=cmd_book,
            events=event_book
        )

        result = ctx.logic.handle(contextual)
        if result.pages:
            result_any = result.pages[0].event
            if result_any.type_url.endswith('TransactionCreated'):
                ctx.result_event = domains.TransactionCreated()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when(parsers.parse('I handle an ApplyDiscount command with type "{discount_type}" and value {value:d}'))
def handle_apply_discount(ctx, discount_type, value):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        cmd = domains.ApplyDiscount(
            discount_type=discount_type,
            value=value,
        )
        cmd_any = Any()
        cmd_any.Pack(cmd)

        cmd_book = angzarr.CommandBook(
            pages=[angzarr.CommandPage(command=cmd_any)]
        )

        contextual = angzarr.ContextualCommand(
            command=cmd_book,
            events=event_book
        )

        result = ctx.logic.handle(contextual)
        if result.pages:
            result_any = result.pages[0].event
            if result_any.type_url.endswith('DiscountApplied'):
                ctx.result_event = domains.DiscountApplied()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when(parsers.parse('I handle a CompleteTransaction command with payment method "{payment_method}"'))
def handle_complete_transaction(ctx, payment_method):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        cmd = domains.CompleteTransaction(
            payment_method=payment_method,
        )
        cmd_any = Any()
        cmd_any.Pack(cmd)

        cmd_book = angzarr.CommandBook(
            pages=[angzarr.CommandPage(command=cmd_any)]
        )

        contextual = angzarr.ContextualCommand(
            command=cmd_book,
            events=event_book
        )

        result = ctx.logic.handle(contextual)
        if result.pages:
            result_any = result.pages[0].event
            if result_any.type_url.endswith('TransactionCompleted'):
                ctx.result_event = domains.TransactionCompleted()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when(parsers.parse('I handle a CancelTransaction command with reason "{reason}"'))
def handle_cancel_transaction(ctx, reason):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        cmd = domains.CancelTransaction(
            reason=reason,
        )
        cmd_any = Any()
        cmd_any.Pack(cmd)

        cmd_book = angzarr.CommandBook(
            pages=[angzarr.CommandPage(command=cmd_any)]
        )

        contextual = angzarr.ContextualCommand(
            command=cmd_book,
            events=event_book
        )

        result = ctx.logic.handle(contextual)
        if result.pages:
            result_any = result.pages[0].event
            if result_any.type_url.endswith('TransactionCancelled'):
                ctx.result_event = domains.TransactionCancelled()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when('I rebuild the transaction state')
def rebuild_transaction_state(ctx):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)


# --- Then steps ---

@then('the result is a TransactionCreated event')
def result_is_transaction_created(ctx):
    assert ctx.error is None, f"Expected result but got error: {ctx.error}"
    assert isinstance(ctx.result_event, domains.TransactionCreated)


@then('the result is a DiscountApplied event')
def result_is_discount_applied(ctx):
    assert ctx.error is None, f"Expected result but got error: {ctx.error}"
    assert isinstance(ctx.result_event, domains.DiscountApplied)


@then('the result is a TransactionCompleted event')
def result_is_transaction_completed(ctx):
    assert ctx.error is None, f"Expected result but got error: {ctx.error}"
    assert isinstance(ctx.result_event, domains.TransactionCompleted)


@then('the result is a TransactionCancelled event')
def result_is_transaction_cancelled(ctx):
    assert ctx.error is None, f"Expected result but got error: {ctx.error}"
    assert isinstance(ctx.result_event, domains.TransactionCancelled)


@then(parsers.parse('the command fails with status "{status}"'))
def command_fails_with_status(ctx, status):
    assert ctx.error is not None, "Expected command to fail but it succeeded"
    # Map status to expected error patterns
    if status == "FAILED_PRECONDITION":
        # Check for precondition errors
        assert any(phrase in str(ctx.error).lower() for phrase in
                   ['already exists', 'does not exist', 'can only', 'pending'])
    elif status == "INVALID_ARGUMENT":
        # Check for validation errors
        assert any(phrase in str(ctx.error).lower() for phrase in
                   ['required', 'at least one', 'must be', 'invalid'])


@then(parsers.parse('the error message contains "{substring}"'))
def error_message_contains(ctx, substring):
    assert ctx.error is not None, "Expected error but command succeeded"
    assert substring.lower() in str(ctx.error).lower(), \
        f"Expected error to contain '{substring}', got '{ctx.error}'"


@then(parsers.parse('the event has customer_id "{customer_id}"'))
def event_has_customer_id(ctx, customer_id):
    assert isinstance(ctx.result_event, domains.TransactionCreated)
    assert ctx.result_event.customer_id == customer_id


@then(parsers.parse('the event has subtotal_cents {subtotal_cents:d}'))
def event_has_subtotal_cents(ctx, subtotal_cents):
    assert isinstance(ctx.result_event, domains.TransactionCreated)
    assert ctx.result_event.subtotal_cents == subtotal_cents


@then(parsers.parse('the event has discount_cents {discount_cents:d}'))
def event_has_discount_cents(ctx, discount_cents):
    assert isinstance(ctx.result_event, domains.DiscountApplied)
    assert ctx.result_event.discount_cents == discount_cents


@then(parsers.parse('the event has final_total_cents {final_total_cents:d}'))
def event_has_final_total_cents(ctx, final_total_cents):
    assert isinstance(ctx.result_event, domains.TransactionCompleted)
    assert ctx.result_event.final_total_cents == final_total_cents


@then(parsers.parse('the event has payment_method "{payment_method}"'))
def event_has_payment_method(ctx, payment_method):
    assert isinstance(ctx.result_event, domains.TransactionCompleted)
    assert ctx.result_event.payment_method == payment_method


@then(parsers.parse('the event has loyalty_points_earned {points:d}'))
def event_has_loyalty_points_earned(ctx, points):
    assert isinstance(ctx.result_event, domains.TransactionCompleted)
    assert ctx.result_event.loyalty_points_earned == points


@then(parsers.parse('the event has reason "{reason}"'))
def event_has_reason(ctx, reason):
    assert isinstance(ctx.result_event, domains.TransactionCancelled)
    assert ctx.result_event.reason == reason


@then(parsers.parse('the state has customer_id "{customer_id}"'))
def state_has_customer_id(ctx, customer_id):
    assert ctx.state is not None
    assert ctx.state.customer_id == customer_id


@then(parsers.parse('the state has subtotal_cents {subtotal_cents:d}'))
def state_has_subtotal_cents(ctx, subtotal_cents):
    assert ctx.state is not None
    assert ctx.state.subtotal_cents == subtotal_cents


@then(parsers.parse('the state has status "{status}"'))
def state_has_status(ctx, status):
    assert ctx.state is not None
    assert ctx.state.status == status
