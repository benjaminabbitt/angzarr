"""Cucumber BDD tests for Customer business logic using pytest-bdd."""

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from google.protobuf.any_pb2 import Any

# Import generated proto (assumes proto is in the path)
import sys
sys.path.insert(0, '.')
from angzarr import angzarr_pb2 as angzarr
from proto import domains_pb2 as domains

from customer_logic import CustomerLogic

# Load scenarios from feature file
scenarios('customer.feature')


class CustomerTestContext:
    """Test context for customer BDD scenarios."""

    def __init__(self):
        self.logic = CustomerLogic()
        self.prior_events = []
        self.result_event = None
        self.error = None
        self.state = None


@pytest.fixture
def ctx():
    """Fixture providing fresh test context for each scenario."""
    return CustomerTestContext()


# --- Given steps ---

@given('no prior events for the aggregate')
def no_prior_events(ctx):
    ctx.prior_events = []


@given(parsers.parse('a CustomerCreated event with name "{name}" and email "{email}"'))
def customer_created_event(ctx, name, email):
    event = domains.CustomerCreated(name=name, email=email)
    event_any = Any()
    event_any.Pack(event)
    ctx.prior_events.append(event_any)


@given(parsers.parse('a LoyaltyPointsAdded event with {points:d} points and new_balance {new_balance:d}'))
def loyalty_points_added_event(ctx, points, new_balance):
    event = domains.LoyaltyPointsAdded(points=points, new_balance=new_balance)
    event_any = Any()
    event_any.Pack(event)
    ctx.prior_events.append(event_any)


@given(parsers.parse('a LoyaltyPointsRedeemed event with {points:d} points and new_balance {new_balance:d}'))
def loyalty_points_redeemed_event(ctx, points, new_balance):
    event = domains.LoyaltyPointsRedeemed(points=points, new_balance=new_balance)
    event_any = Any()
    event_any.Pack(event)
    ctx.prior_events.append(event_any)


# --- Helper to build event book ---

def build_event_book(prior_events):
    if not prior_events:
        return None
    pages = [angzarr.EventPage(event=event) for event in prior_events]
    return angzarr.EventBook(pages=pages)


# --- When steps ---

@when(parsers.re(r'I handle a CreateCustomer command with name "(?P<name>[^"]*)" and email "(?P<email>[^"]*)"'))
def handle_create_customer(ctx, name, email):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        # Create command
        cmd = domains.CreateCustomer(name=name, email=email)
        cmd_any = Any()
        cmd_any.Pack(cmd)

        # Build command book
        cmd_book = angzarr.CommandBook(
            pages=[angzarr.CommandPage(command=cmd_any)]
        )

        # Build contextual command
        contextual = angzarr.ContextualCommand(
            command=cmd_book,
            events=event_book
        )

        result = ctx.logic.handle(contextual)
        # Extract the event from result
        if result.pages:
            result_any = result.pages[0].event
            if result_any.type_url.endswith('CustomerCreated'):
                ctx.result_event = domains.CustomerCreated()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when(parsers.parse('I handle an AddLoyaltyPoints command with {points:d} points and reason "{reason}"'))
def handle_add_loyalty_points(ctx, points, reason):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        cmd = domains.AddLoyaltyPoints(points=points, reason=reason)
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
            if result_any.type_url.endswith('LoyaltyPointsAdded'):
                ctx.result_event = domains.LoyaltyPointsAdded()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when(parsers.parse('I handle a RedeemLoyaltyPoints command with {points:d} points and type "{redemption_type}"'))
def handle_redeem_loyalty_points(ctx, points, redemption_type):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)

    try:
        cmd = domains.RedeemLoyaltyPoints(points=points, redemption_type=redemption_type)
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
            if result_any.type_url.endswith('LoyaltyPointsRedeemed'):
                ctx.result_event = domains.LoyaltyPointsRedeemed()
                result_any.Unpack(ctx.result_event)
        ctx.error = None
    except ValueError as e:
        ctx.error = e
        ctx.result_event = None


@when('I rebuild the customer state')
def rebuild_customer_state(ctx):
    event_book = build_event_book(ctx.prior_events)
    ctx.state = ctx.logic._rebuild_state(event_book)


# --- Then steps ---

@then('the result is a CustomerCreated event')
def result_is_customer_created(ctx):
    assert ctx.error is None, f"Expected result but got error: {ctx.error}"
    assert isinstance(ctx.result_event, domains.CustomerCreated)


@then('the result is a LoyaltyPointsAdded event')
def result_is_loyalty_points_added(ctx):
    assert ctx.error is None, f"Expected result but got error: {ctx.error}"
    assert isinstance(ctx.result_event, domains.LoyaltyPointsAdded)


@then('the result is a LoyaltyPointsRedeemed event')
def result_is_loyalty_points_redeemed(ctx):
    assert ctx.error is None, f"Expected result but got error: {ctx.error}"
    assert isinstance(ctx.result_event, domains.LoyaltyPointsRedeemed)


@then(parsers.parse('the command fails with status "{status}"'))
def command_fails_with_status(ctx, status):
    assert ctx.error is not None, "Expected command to fail but it succeeded"
    # Map status to expected error patterns
    if status == "FAILED_PRECONDITION":
        # Check for precondition errors
        assert any(phrase in str(ctx.error).lower() for phrase in
                   ['already exists', 'does not exist', 'insufficient'])
    elif status == "INVALID_ARGUMENT":
        # Check for validation errors
        assert any(phrase in str(ctx.error).lower() for phrase in
                   ['required', 'must be positive', 'invalid'])


@then(parsers.parse('the error message contains "{substring}"'))
def error_message_contains(ctx, substring):
    assert ctx.error is not None, "Expected error but command succeeded"
    assert substring.lower() in str(ctx.error).lower(), \
        f"Expected error to contain '{substring}', got '{ctx.error}'"


@then(parsers.parse('the event has name "{name}"'))
def event_has_name(ctx, name):
    assert isinstance(ctx.result_event, domains.CustomerCreated)
    assert ctx.result_event.name == name


@then(parsers.parse('the event has email "{email}"'))
def event_has_email(ctx, email):
    assert isinstance(ctx.result_event, domains.CustomerCreated)
    assert ctx.result_event.email == email


@then(parsers.parse('the event has points {points:d}'))
def event_has_points(ctx, points):
    if isinstance(ctx.result_event, domains.LoyaltyPointsAdded):
        assert ctx.result_event.points == points
    elif isinstance(ctx.result_event, domains.LoyaltyPointsRedeemed):
        assert ctx.result_event.points == points
    else:
        raise AssertionError(f"Expected points event, got {type(ctx.result_event)}")


@then(parsers.parse('the event has new_balance {new_balance:d}'))
def event_has_new_balance(ctx, new_balance):
    if isinstance(ctx.result_event, domains.LoyaltyPointsAdded):
        assert ctx.result_event.new_balance == new_balance
    elif isinstance(ctx.result_event, domains.LoyaltyPointsRedeemed):
        assert ctx.result_event.new_balance == new_balance
    else:
        raise AssertionError(f"Expected points event, got {type(ctx.result_event)}")


@then(parsers.parse('the event has reason "{reason}"'))
def event_has_reason(ctx, reason):
    assert isinstance(ctx.result_event, domains.LoyaltyPointsAdded)
    assert ctx.result_event.reason == reason


@then(parsers.parse('the event has redemption_type "{redemption_type}"'))
def event_has_redemption_type(ctx, redemption_type):
    assert isinstance(ctx.result_event, domains.LoyaltyPointsRedeemed)
    assert ctx.result_event.redemption_type == redemption_type


@then(parsers.parse('the state has name "{name}"'))
def state_has_name(ctx, name):
    assert ctx.state is not None
    assert ctx.state.name == name


@then(parsers.parse('the state has email "{email}"'))
def state_has_email(ctx, email):
    assert ctx.state is not None
    assert ctx.state.email == email


@then(parsers.parse('the state has loyalty_points {points:d}'))
def state_has_loyalty_points(ctx, points):
    assert ctx.state is not None
    assert ctx.state.loyalty_points == points


@then(parsers.parse('the state has lifetime_points {points:d}'))
def state_has_lifetime_points(ctx, points):
    assert ctx.state is not None
    assert ctx.state.lifetime_points == points
