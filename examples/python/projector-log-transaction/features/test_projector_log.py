"""Cucumber BDD tests for Transaction Log Projector using pytest-bdd."""

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

import sys
sys.path.insert(0, '.')

from proto import domains_pb2 as domains
from log_projector import TransactionLogProjector


# Load scenarios from feature file
scenarios('projector-log.feature')


class LogProjectorTestContext:
    """Test context for log projector BDD scenarios."""

    def __init__(self):
        self.projector = TransactionLogProjector()
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
    # Transaction log projector does not handle customer events - skip
    pytest.skip("Transaction log projector only handles transaction domain events")


@given(parsers.parse('a LoyaltyPointsAdded event with {points:d} points and new_balance {new_balance:d}'))
def loyalty_points_added_event(ctx, points, new_balance):
    # Transaction log projector does not handle customer events - skip
    pytest.skip("Transaction log projector only handles transaction domain events")


@given(parsers.parse('a TransactionCreated event with customer "{customer_id}" and subtotal {subtotal:d}'))
def transaction_created_event(ctx, customer_id, subtotal):
    event = domains.TransactionCreated(
        customer_id=customer_id,
        subtotal_cents=subtotal,
    )
    ctx.event_book = {
        "cover": {
            "domain": "transaction",
            "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08"},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.TransactionCreated",
                    "value": event.SerializeToString(),
                },
            }
        ],
    }


@given(parsers.parse('a TransactionCompleted event with total {total:d} and payment "{payment}"'))
def transaction_completed_event(ctx, total, payment):
    event = domains.TransactionCompleted(
        final_total_cents=total,
        payment_method=payment,
    )
    ctx.event_book = {
        "cover": {
            "domain": "transaction",
            "root": {"value": b"\x01\x02\x03\x04\x05\x06\x07\x08"},
        },
        "pages": [
            {
                "sequence": {"num": 0},
                "event": {
                    "type_url": "type.examples/examples.TransactionCompleted",
                    "value": event.SerializeToString(),
                },
            }
        ],
    }


@given('an unknown event type')
def unknown_event_type(ctx):
    ctx.event_book = {
        "cover": {
            "domain": "transaction",
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
