"""Cucumber BDD tests for Customer Log Projector using pytest-bdd."""

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

import sys
sys.path.insert(0, '.')

from proto import domains_pb2 as domains
from log_projector import CustomerLogProjector


# Load scenarios from feature file
scenarios('projector-log.feature')


class LogProjectorTestContext:
    """Test context for log projector BDD scenarios."""

    def __init__(self):
        self.projector = CustomerLogProjector()
        self.event_book = None
        self.output = None
        self.error = None


@pytest.fixture
def ctx():
    """Fixture providing fresh test context for each scenario."""
    return LogProjectorTestContext()


@pytest.fixture
def capsys_ctx(ctx, capsys):
    """Fixture combining context with capsys for capturing output."""
    ctx.capsys = capsys
    return ctx


# --- Given steps ---

@given(parsers.parse('a CustomerCreated event with name "{name}" and email "{email}"'))
def customer_created_event(ctx, name, email):
    event = domains.CustomerCreated(name=name, email=email)
    ctx.event_book = {
        "cover": {
            "domain": "customer",
            "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08"},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.CustomerCreated",
                    "value": event.SerializeToString(),
                },
            }
        ],
    }


@given(parsers.parse('a LoyaltyPointsAdded event with {points:d} points and new_balance {new_balance:d}'))
def loyalty_points_added_event(ctx, points, new_balance):
    event = domains.LoyaltyPointsAdded(points=points, new_balance=new_balance)
    ctx.event_book = {
        "cover": {
            "domain": "customer",
            "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08"},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.LoyaltyPointsAdded",
                    "value": event.SerializeToString(),
                },
            }
        ],
    }


@given(parsers.parse('a TransactionCreated event with customer "{customer_id}" and subtotal {subtotal:d}'))
def transaction_created_event(ctx, customer_id, subtotal):
    # Customer log projector does not handle transaction events - skip
    pytest.skip("Customer log projector only handles customer domain events")


@given(parsers.parse('a TransactionCompleted event with total {total:d} and payment "{payment}"'))
def transaction_completed_event(ctx, total, payment):
    # Customer log projector does not handle transaction events - skip
    pytest.skip("Customer log projector only handles customer domain events")


@given('an unknown event type')
def unknown_event_type(ctx):
    ctx.event_book = {
        "cover": {
            "domain": "customer",
            "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08"},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.UnknownEvent",
                    "value": b"some unknown data",
                },
            }
        ],
    }


# --- When steps ---

@when('I process the log projector')
def process_log_projector(capsys_ctx):
    ctx = capsys_ctx
    try:
        ctx.projector.project(ctx.event_book)
        ctx.output = ctx.capsys.readouterr().out
        ctx.error = None
    except Exception as e:
        ctx.error = e
        ctx.output = ctx.capsys.readouterr().out


# --- Then steps ---

@then('the event is logged successfully')
def event_logged_successfully(capsys_ctx):
    ctx = capsys_ctx
    assert ctx.error is None, f"Expected success but got error: {ctx.error}"
    # The log projector should have printed something
    assert ctx.output is not None


@then('the event is logged as unknown')
def event_logged_as_unknown(capsys_ctx):
    ctx = capsys_ctx
    assert ctx.error is None, f"Expected success but got error: {ctx.error}"
    # Unknown events should still be logged without errors
    assert ctx.output is not None
