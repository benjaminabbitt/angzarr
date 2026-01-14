"""pytest-bdd step definitions for Loyalty Saga feature tests."""

import pytest
from pytest_bdd import given, when, then, scenarios, parsers

import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent.parent))

from proto import domains_pb2 as domains
from loyalty_saga import LoyaltyPointsSaga, CommandBook


# Load all scenarios from the feature file
scenarios("saga-loyalty.feature")


@pytest.fixture
def saga():
    """Create a fresh saga instance for each test."""
    return LoyaltyPointsSaga()


@pytest.fixture
def context():
    """Shared context for step definitions."""
    return {"event_book": None, "commands": None}


@given(
    parsers.parse('a TransactionCreated event with customer "{customer_id}" and subtotal {subtotal:d}'),
    target_fixture="context",
)
def transaction_created_event(context, customer_id, subtotal):
    """Create a TransactionCreated event (not TransactionCompleted)."""
    transaction_created = domains.TransactionCreated(
        customer_id=customer_id,
        subtotal_cents=subtotal,
    )

    context["event_book"] = {
        "cover": {
            "domain": "transaction",
            "root": {"value": bytes.fromhex("0123456789abcdef0123456789abcdef")},
        },
        "pages": [
            {
                "event": {
                    "type_url": "type.examples/examples.TransactionCreated",
                    "value": transaction_created.SerializeToString(),
                }
            }
        ],
    }
    return context


@given(
    parsers.parse("a TransactionCompleted event with {points:d} loyalty points earned"),
    target_fixture="context",
)
def transaction_completed_event(context, points):
    """Create a TransactionCompleted event with specified points."""
    transaction_completed = domains.TransactionCompleted(
        final_total_cents=10000,
        payment_method="card",
        loyalty_points_earned=points,
    )

    context["event_book"] = {
        "cover": {
            "domain": "transaction",
            "root": {"value": bytes.fromhex("0123456789abcdef0123456789abcdef")},
        },
        "pages": [
            {
                "event": {
                    "type_url": "type.examples/examples.TransactionCompleted",
                    "value": transaction_completed.SerializeToString(),
                }
            }
        ],
    }
    return context


@when("I process the saga")
def process_saga(saga, context):
    """Process the event book through the saga."""
    context["commands"] = saga.handle(context["event_book"])


@then("no commands are generated")
def no_commands_generated(context):
    """Verify no commands were generated."""
    assert len(context["commands"]) == 0


@then("an AddLoyaltyPoints command is generated")
def add_loyalty_points_command_generated(context):
    """Verify an AddLoyaltyPoints command was generated."""
    assert len(context["commands"]) == 1
    cmd = context["commands"][0]
    assert "AddLoyaltyPoints" in cmd.command_type


@then(parsers.parse("the command has points {points:d}"))
def command_has_points(context, points):
    """Verify the command has the expected points value."""
    cmd = context["commands"][0]
    add_points = domains.AddLoyaltyPoints()
    add_points.ParseFromString(cmd.command_data)
    assert add_points.points == points


@then(parsers.parse('the command has domain "{domain}"'))
def command_has_domain(context, domain):
    """Verify the command targets the expected domain."""
    cmd = context["commands"][0]
    assert cmd.domain == domain


@then(parsers.parse('the command reason contains "{text}"'))
def command_reason_contains(context, text):
    """Verify the command reason contains the expected text."""
    cmd = context["commands"][0]
    add_points = domains.AddLoyaltyPoints()
    add_points.ParseFromString(cmd.command_data)
    assert text in add_points.reason
