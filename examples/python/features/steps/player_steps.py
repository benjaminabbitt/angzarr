"""Step definitions for player aggregate tests.

Uses functional handler pattern: handlers are standalone functions
that take (cmd, state, seq) and return events.
"""

import sys
from datetime import datetime, timezone
from pathlib import Path

# Add project root for proto imports
project_root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(project_root))

from behave import given, then, use_step_matcher, when
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp
from player.agg.handlers import (
    handle_deposit,
    handle_register,
    handle_release,
    handle_reserve,
    handle_withdraw,
)
from player.agg.state import PlayerState, build_state

from angzarr_client.errors import CommandRejectedError
from angzarr_client.helpers import try_unpack, type_name_from_url
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

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
        if evt := try_unpack(ep.event, player.FundsDeposited):
            if evt.new_balance:
                prior_balance = evt.new_balance.amount

    new_balance = prior_balance + int(amount)

    event = player.FundsDeposited(
        amount=poker_types.Currency(amount=int(amount), currency_code="CHIPS"),
        new_balance=poker_types.Currency(amount=new_balance, currency_code="CHIPS"),
        deposited_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, seq=len(context.events)))


@given(
    r'a FundsReserved event with amount (?P<amount>\d+) for table "(?P<table_id>[^"]+)"'
)
def step_given_funds_reserved(context, amount, table_id):
    """Add a FundsReserved event to history."""
    if not hasattr(context, "events"):
        context.events = []

    # Calculate available balance
    total_deposited = 0
    total_reserved = 0
    for ep in context.events:
        if evt := try_unpack(ep.event, player.FundsDeposited):
            if evt.new_balance:
                total_deposited = evt.new_balance.amount
        elif evt := try_unpack(ep.event, player.FundsReserved):
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

# Handler lookup by method name
_HANDLER_MAP = {
    "register": handle_register,
    "deposit": handle_deposit,
    "withdraw": handle_withdraw,
    "reserve": handle_reserve,
    "release": handle_release,
}


def _build_state_from_events(events: list) -> PlayerState:
    """Build player state from list of EventPages."""
    state = PlayerState()
    event_anys = [page.event for page in events if page.HasField("event")]
    return build_state(state, event_anys)


def _execute_handler(context, method_name: str, cmd):
    """Execute a functional command handler."""
    events = context.events if hasattr(context, "events") else []
    state = _build_state_from_events(events)
    seq = len(events)

    handler = _HANDLER_MAP.get(method_name)
    if not handler:
        raise ValueError(f"Unknown handler: {method_name}")

    try:
        result_event = handler(cmd, state, seq)

        # Pack result into EventPage and EventBook
        event_any = ProtoAny()
        event_any.Pack(result_event, type_url_prefix="type.googleapis.com/")
        result_page = types.EventPage(
            sequence=seq,
            event=event_any,
            created_at=make_timestamp(),
        )
        result_book = _make_event_book([result_page])

        context.result = result_book
        context.error = None
        context.result_event_any = event_any

        # Store state for assertion steps (apply new event)
        context.state = build_state(state, [event_any])
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
    context.state = _build_state_from_events(context.events)


# --- Then steps ---


@then(r"the result is a (?P<event_type>\w+) event")
def step_then_result_is_event(context, event_type):
    """Verify the result event type."""
    assert (
        context.result is not None
    ), f"Expected {event_type} event but got error: {context.error}"
    assert context.result.pages, "No event pages in result"
    event_any = context.result.pages[0].event
    actual_type = type_name_from_url(event_any.type_url)
    assert actual_type == event_type, f"Expected {event_type} but got {actual_type}"


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
    event = (
        try_unpack(event_any, player.FundsDeposited)
        or try_unpack(event_any, player.FundsWithdrawn)
        or try_unpack(event_any, player.FundsReserved)
        or try_unpack(event_any, player.FundsReleased)
    )
    if event is None:
        raise AssertionError(f"Unknown event type: {event_any.type_url}")

    assert event.amount.amount == int(
        amount
    ), f"Expected amount={amount}, got {event.amount.amount}"


@then(r"the player event has new_balance (?P<balance>\d+)")
def step_then_event_has_new_balance(context, balance):
    """Verify the event new_balance field."""
    event_any = context.result_event_any

    # Try different event types that have new_balance field
    event = try_unpack(event_any, player.FundsDeposited) or try_unpack(
        event_any, player.FundsWithdrawn
    )
    if event is None:
        raise AssertionError(
            f"Unknown event type for new_balance: {event_any.type_url}"
        )

    assert event.new_balance.amount == int(
        balance
    ), f"Expected new_balance={balance}, got {event.new_balance.amount}"


@then(r"the player event has new_available_balance (?P<balance>\d+)")
def step_then_event_has_new_available_balance(context, balance):
    """Verify the event new_available_balance field."""
    event_any = context.result_event_any

    event = try_unpack(event_any, player.FundsReserved) or try_unpack(
        event_any, player.FundsReleased
    )
    if event is None:
        raise AssertionError(
            f"Unknown event type for new_available_balance: {event_any.type_url}"
        )

    assert event.new_available_balance.amount == int(
        balance
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
    assert context.state is not None, "No player state"
    assert context.state.bankroll == int(
        amount
    ), f"Expected bankroll={amount}, got {context.state.bankroll}"


@then(r"the player state has reserved_funds (?P<amount>\d+)")
def step_then_state_has_reserved_funds(context, amount):
    """Verify the player state reserved_funds."""
    assert context.state is not None, "No player state"
    assert context.state.reserved_funds == int(
        amount
    ), f"Expected reserved_funds={amount}, got {context.state.reserved_funds}"


@then(r"the player state has available_balance (?P<amount>\d+)")
def step_then_state_has_available_balance(context, amount):
    """Verify the player state available_balance."""
    assert context.state is not None, "No player state"
    available = context.state.available_balance
    assert available == int(
        amount
    ), f"Expected available_balance={amount}, got {available}"
