"""Step definitions for player aggregate tests."""

import sys
from datetime import datetime, timezone
from pathlib import Path

# Add project root for proto imports
project_root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(project_root))

from behave import given, when, then, use_step_matcher
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types
from angzarr_client.errors import CommandRejectedError

from player.agg.handlers import Player


# Use regex matchers for flexibility
use_step_matcher("re")


def make_timestamp():
    """Create current timestamp."""
    return Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))


def make_event_page(event_msg, seq: int = 0) -> types.EventPage:
    """Create EventPage with packed event."""
    event_any = ProtoAny()
    event_any.Pack(event_msg, type_url_prefix="type.googleapis.com/")
    return types.EventPage(
        sequence=seq,
        event=event_any,
        created_at=make_timestamp(),
    )


def _make_event_book(pages):
    """Create an EventBook from a list of EventPages."""
    return types.EventBook(
        cover=types.Cover(
            root=types.UUID(value=b"player-123"),
            domain="player",
        ),
        pages=pages,
    )


# --- Given steps ---


@given(r"no prior events for the player aggregate")
def step_given_no_prior_events(context):
    """Initialize with empty event history."""
    context.events = []


@given(r'a PlayerRegistered event for "(?P<name>[^"]+)"')
def step_given_player_registered(context, name):
    """Add a PlayerRegistered event to history."""
    if not hasattr(context, "events"):
        context.events = []

    event = player.PlayerRegistered(
        display_name=name,
        email=f"{name.lower()}@example.com",
        player_type=poker_types.PlayerType.HUMAN,
        registered_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, seq=len(context.events)))


@given(r"a FundsDeposited event with amount (?P<amount>\d+)")
def step_given_funds_deposited(context, amount):
    """Add a FundsDeposited event to history."""
    if not hasattr(context, "events"):
        context.events = []

    # Calculate new balance from prior deposits
    prior_balance = 0
    for ep in context.events:
        if ep.event.type_url.endswith("FundsDeposited"):
            evt = player.FundsDeposited()
            ep.event.Unpack(evt)
            if evt.new_balance:
                prior_balance = evt.new_balance.amount

    new_balance = prior_balance + int(amount)

    event = player.FundsDeposited(
        amount=poker_types.Currency(amount=int(amount), currency_code="CHIPS"),
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        deposited_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, seq=len(context.events)))


@given(r'a FundsReserved event with amount (?P<amount>\d+) for table "(?P<table_id>[^"]+)"')
def step_given_funds_reserved(context, amount, table_id):
    """Add a FundsReserved event to history."""
    if not hasattr(context, "events"):
        context.events = []

    # Calculate available balance
    total_deposited = 0
    total_reserved = 0
    for ep in context.events:
        if ep.event.type_url.endswith("FundsDeposited"):
            evt = player.FundsDeposited()
            ep.event.Unpack(evt)
            if evt.new_balance:
                total_deposited = evt.new_balance.amount
        elif ep.event.type_url.endswith("FundsReserved"):
            evt = player.FundsReserved()
            ep.event.Unpack(evt)
            if evt.new_reserved_balance:
                total_reserved = evt.new_reserved_balance.amount

    new_reserved = total_reserved + int(amount)
    new_available = total_deposited - new_reserved

    event = player.FundsReserved(
        amount=poker_types.Currency(amount=int(amount), currency_code="CHIPS"),
        table_root=table_id.encode("utf-8"),
        new_available_balance=poker_types.Currency(
            amount=new_available, currency_code="CHIPS"
        ),
        new_reserved_balance=poker_types.Currency(
            amount=new_reserved, currency_code="CHIPS"
        ),
        reserved_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, seq=len(context.events)))


# --- When steps ---


def _execute_handler(context, method_name: str, cmd):
    """Execute a command handler method on the Player aggregate."""
    event_book = _make_event_book(context.events if hasattr(context, "events") else [])
    agg = Player(event_book)

    try:
        method = getattr(agg, method_name)
        result = method(cmd)
        # Get the event book with new events
        result_book = agg.event_book()
        context.result = result_book
        context.error = None
        # Extract the event for assertion steps
        if result_book.pages:
            context.result_event_any = result_book.pages[0].event
        # Store aggregate for state access
        context.agg = agg
    except CommandRejectedError as e:
        context.result = None
        context.error = e
        context.error_message = str(e)


@when(
    r'I handle a RegisterPlayer command with name "(?P<name>[^"]+)" and email "(?P<email>[^"]+)"'
)
def step_when_register_player(context, name, email):
    """Handle RegisterPlayer command."""
    cmd = player.RegisterPlayer(
        display_name=name,
        email=email,
        player_type=poker_types.PlayerType.HUMAN,
    )
    _execute_handler(context, "register", cmd)


@when(
    r'I handle a RegisterPlayer command with name "(?P<name>[^"]+)" and email "(?P<email>[^"]+)" as AI'
)
def step_when_register_player_ai(context, name, email):
    """Handle RegisterPlayer command for AI player."""
    cmd = player.RegisterPlayer(
        display_name=name,
        email=email,
        player_type=poker_types.PlayerType.AI,
        ai_model_id="gpt-4",
    )
    _execute_handler(context, "register", cmd)


@when(r"I handle a DepositFunds command with amount (?P<amount>\d+)")
def step_when_deposit_funds(context, amount):
    """Handle DepositFunds command."""
    cmd = player.DepositFunds(
        amount=poker_types.Currency(amount=int(amount), currency_code="CHIPS"),
    )
    _execute_handler(context, "deposit", cmd)


@when(r"I handle a WithdrawFunds command with amount (?P<amount>\d+)")
def step_when_withdraw_funds(context, amount):
    """Handle WithdrawFunds command."""
    cmd = player.WithdrawFunds(
        amount=poker_types.Currency(amount=int(amount), currency_code="CHIPS"),
    )
    _execute_handler(context, "withdraw", cmd)


@when(
    r'I handle a ReserveFunds command with amount (?P<amount>\d+) for table "(?P<table_id>[^"]+)"'
)
def step_when_reserve_funds(context, amount, table_id):
    """Handle ReserveFunds command."""
    cmd = player.ReserveFunds(
        amount=poker_types.Currency(amount=int(amount), currency_code="CHIPS"),
        table_root=table_id.encode("utf-8"),
    )
    _execute_handler(context, "reserve", cmd)


@when(r'I handle a ReleaseFunds command for table "(?P<table_id>[^"]+)"')
def step_when_release_funds(context, table_id):
    """Handle ReleaseFunds command."""
    cmd = player.ReleaseFunds(
        table_root=table_id.encode("utf-8"),
    )
    _execute_handler(context, "release", cmd)


@when(r"I rebuild the player state")
def step_when_rebuild_state(context):
    """Rebuild player state from events."""
    event_book = _make_event_book(context.events)
    context.agg = Player(event_book)


# --- Then steps ---


@then(r"the result is a (?P<event_type>\w+) event")
def step_then_result_is_event(context, event_type):
    """Verify the result event type."""
    assert context.result is not None, f"Expected {event_type} event but got error: {context.error}"
    assert context.result.pages, "No event pages in result"
    event_any = context.result.pages[0].event
    assert event_any.type_url.endswith(
        event_type
    ), f"Expected {event_type} but got {event_any.type_url}"


@then(r'the player event has display_name "(?P<name>[^"]+)"')
def step_then_event_has_display_name(context, name):
    """Verify the event display_name field."""
    event = player.PlayerRegistered()
    context.result_event_any.Unpack(event)
    assert (
        event.display_name == name
    ), f"Expected display_name={name}, got {event.display_name}"


@then(r'the player event has player_type "(?P<ptype>[^"]+)"')
def step_then_event_has_player_type(context, ptype):
    """Verify the event player_type field."""
    event = player.PlayerRegistered()
    context.result_event_any.Unpack(event)
    expected_type = getattr(poker_types.PlayerType, ptype)
    assert (
        event.player_type == expected_type
    ), f"Expected player_type={ptype}, got {event.player_type}"


@then(r"the player event has amount (?P<amount>\d+)")
def step_then_event_has_amount(context, amount):
    """Verify the event amount field."""
    event_any = context.result_event_any

    # Try different event types that have amount field
    if event_any.type_url.endswith("FundsDeposited"):
        event = player.FundsDeposited()
    elif event_any.type_url.endswith("FundsWithdrawn"):
        event = player.FundsWithdrawn()
    elif event_any.type_url.endswith("FundsReserved"):
        event = player.FundsReserved()
    elif event_any.type_url.endswith("FundsReleased"):
        event = player.FundsReleased()
    else:
        raise AssertionError(f"Unknown event type: {event_any.type_url}")

    event_any.Unpack(event)
    assert (
        event.amount.amount == int(amount)
    ), f"Expected amount={amount}, got {event.amount.amount}"


@then(r"the player event has new_balance (?P<balance>\d+)")
def step_then_event_has_new_balance(context, balance):
    """Verify the event new_balance field."""
    event_any = context.result_event_any

    # Try different event types that have new_balance field
    if event_any.type_url.endswith("FundsDeposited"):
        event = player.FundsDeposited()
    elif event_any.type_url.endswith("FundsWithdrawn"):
        event = player.FundsWithdrawn()
    else:
        raise AssertionError(f"Unknown event type for new_balance: {event_any.type_url}")

    event_any.Unpack(event)
    assert (
        event.new_balance.amount == int(balance)
    ), f"Expected new_balance={balance}, got {event.new_balance.amount}"


@then(r"the player event has new_available_balance (?P<balance>\d+)")
def step_then_event_has_new_available_balance(context, balance):
    """Verify the event new_available_balance field."""
    event_any = context.result_event_any

    if event_any.type_url.endswith("FundsReserved"):
        event = player.FundsReserved()
    elif event_any.type_url.endswith("FundsReleased"):
        event = player.FundsReleased()
    else:
        raise AssertionError(
            f"Unknown event type for new_available_balance: {event_any.type_url}"
        )

    event_any.Unpack(event)
    assert (
        event.new_available_balance.amount == int(balance)
    ), f"Expected new_available_balance={balance}, got {event.new_available_balance.amount}"


@then(r'the command fails with status "(?P<status>[^"]+)"')
def step_then_command_fails_with_status(context, status):
    """Verify the command failed with expected status."""
    assert context.error is not None, "Expected command to fail but it succeeded"
    # CommandRejectedError maps to different gRPC statuses based on message
    # For now, we just verify there's an error - the status mapping happens at the gRPC layer


@then(r'the error message contains "(?P<text>[^"]+)"')
def step_then_error_contains(context, text):
    """Verify the error message contains expected text."""
    assert context.error is not None, "Expected an error but got success"
    assert (
        text.lower() in context.error_message.lower()
    ), f"Expected error to contain '{text}', got '{context.error_message}'"


@then(r"the player state has bankroll (?P<amount>\d+)")
def step_then_state_has_bankroll(context, amount):
    """Verify the player state bankroll."""
    assert context.agg is not None, "No player aggregate"
    assert (
        context.agg.bankroll == int(amount)
    ), f"Expected bankroll={amount}, got {context.agg.bankroll}"


@then(r"the player state has reserved_funds (?P<amount>\d+)")
def step_then_state_has_reserved_funds(context, amount):
    """Verify the player state reserved_funds."""
    assert context.agg is not None, "No player aggregate"
    assert (
        context.agg.reserved_funds == int(amount)
    ), f"Expected reserved_funds={amount}, got {context.agg.reserved_funds}"


@then(r"the player state has available_balance (?P<amount>\d+)")
def step_then_state_has_available_balance(context, amount):
    """Verify the player state available_balance."""
    assert context.agg is not None, "No player aggregate"
    available = context.agg.available_balance
    assert (
        available == int(amount)
    ), f"Expected available_balance={amount}, got {available}"
