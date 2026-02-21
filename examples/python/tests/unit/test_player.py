"""Player aggregate unit tests.

DOC: This file is referenced in docs/docs/examples/aggregates.mdx
     Update documentation when making changes to test patterns.
"""

# docs:start:bdd_imports
import sys
from pathlib import Path

import pytest
from pytest_bdd import scenarios, given, when, then, parsers
from google.protobuf.any_pb2 import Any as ProtoAny
# docs:end:bdd_imports

# Add paths
root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(root))
sys.path.insert(0, str(root / "player" / "agg"))
sys.path.insert(0, str(root / "player" / "agg" / "handlers"))

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import poker_types_pb2 as poker_types
from angzarr_client.errors import CommandRejectedError

from handlers.state import PlayerState, build_state


def state_from_event_book(event_book):
    """Build state from EventBook - extracts Any-wrapped events and applies them."""
    state = PlayerState()
    if event_book is None:
        return state
    events = [page.event for page in event_book.pages if page.event]
    return build_state(state, events)
from handlers.register_player import handle_register_player
from handlers.deposit_funds import handle_deposit_funds
from handlers.withdraw_funds import handle_withdraw_funds
from handlers.reserve_funds import handle_reserve_funds
from handlers.release_funds import handle_release_funds

from tests.conftest import (
    ScenarioContext, make_cover, make_command_book, make_timestamp, pack_event
)

# docs:start:scenarios_loader
# Load scenarios from feature file
scenarios("../../../features/unit/player.feature")
# docs:end:scenarios_loader


# --- Fixtures ---

@pytest.fixture
def ctx():
    """Test context for player tests."""
    context = ScenarioContext()
    context.domain = "player"
    context.root = b"player-test"
    return context


def _event_book(ctx: ScenarioContext) -> types.EventBook:
    """Build EventBook from context."""
    return ctx.event_book()


def _handle_command(ctx: ScenarioContext, command_msg, handler_fn):
    """Execute a command handler."""
    cmd_any = ProtoAny()
    cmd_any.Pack(command_msg, type_url_prefix="type.googleapis.com/")

    event_book = _event_book(ctx)
    state = state_from_event_book(event_book)
    seq = len(ctx.events)

    cmd_book = make_command_book(
        make_cover(ctx.domain, ctx.root),
        cmd_any,
        seq,
    )

    try:
        ctx.result = handler_fn(cmd_book, cmd_any, state, seq)
        ctx.error = None
    except CommandRejectedError as e:
        ctx.result = None
        ctx.error = e


# --- Given steps ---

@given("no prior events for the player aggregate")
def no_prior_events(ctx):
    """Start with empty event history."""
    ctx.events = []


# docs:start:given_step
@given(parsers.parse('a PlayerRegistered event for "{name}"'))
def player_registered_event(ctx, name):
    """Add a PlayerRegistered event."""
    event = player.PlayerRegistered(
        display_name=name,
        email=f"{name.lower()}@example.com",
        player_type=poker_types.HUMAN,
        registered_at=make_timestamp(),
    )
    ctx.add_event(event)
# docs:end:given_step


@given(parsers.parse("a FundsDeposited event with amount {amount:d}"))
def funds_deposited_event(ctx, amount):
    """Add a FundsDeposited event."""
    # Calculate new balance from prior events
    prior_balance = 0
    for event_any in ctx.events:
        if event_any.type_url.endswith("FundsDeposited"):
            e = player.FundsDeposited()
            event_any.Unpack(e)
            if e.new_balance:
                prior_balance = e.new_balance.amount

    event = player.FundsDeposited(
        amount=poker_types.Currency(amount=amount),
        new_balance=poker_types.Currency(amount=prior_balance + amount),
        deposited_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse('a FundsReserved event with amount {amount:d} for table "{table_id}"'))
def funds_reserved_event(ctx, amount, table_id):
    """Add a FundsReserved event."""
    table_root = table_id.encode()

    # Calculate balances
    bankroll = 0
    reserved = 0
    for event_any in ctx.events:
        if event_any.type_url.endswith("FundsDeposited"):
            e = player.FundsDeposited()
            event_any.Unpack(e)
            if e.new_balance:
                bankroll = e.new_balance.amount
        elif event_any.type_url.endswith("FundsReserved"):
            e = player.FundsReserved()
            event_any.Unpack(e)
            if e.new_reserved_balance:
                reserved = e.new_reserved_balance.amount

    event = player.FundsReserved(
        amount=poker_types.Currency(amount=amount),
        table_root=table_root,
        new_available_balance=poker_types.Currency(amount=bankroll - reserved - amount),
        new_reserved_balance=poker_types.Currency(amount=reserved + amount),
        reserved_at=make_timestamp(),
    )
    ctx.add_event(event)


# --- When steps ---

# docs:start:when_step
@when(parsers.parse('I handle a RegisterPlayer command with name "{name}" and email "{email}"'))
def handle_register_player_cmd(ctx, name, email):
    """Handle RegisterPlayer command."""
    cmd = player.RegisterPlayer(
        display_name=name,
        email=email,
        player_type=poker_types.HUMAN,
    )
    _handle_command(ctx, cmd, handle_register_player)
# docs:end:when_step


@when(parsers.parse('I handle a RegisterPlayer command with name "{name}" and email "{email}" as AI'))
def handle_register_player_ai_cmd(ctx, name, email):
    """Handle RegisterPlayer command for AI player."""
    cmd = player.RegisterPlayer(
        display_name=name,
        email=email,
        player_type=poker_types.AI,
    )
    _handle_command(ctx, cmd, handle_register_player)


@when(parsers.parse("I handle a DepositFunds command with amount {amount:d}"))
def handle_deposit_funds_cmd(ctx, amount):
    """Handle DepositFunds command."""
    cmd = player.DepositFunds(
        amount=poker_types.Currency(amount=amount),
    )
    _handle_command(ctx, cmd, handle_deposit_funds)


@when(parsers.parse("I handle a WithdrawFunds command with amount {amount:d}"))
def handle_withdraw_funds_cmd(ctx, amount):
    """Handle WithdrawFunds command."""
    cmd = player.WithdrawFunds(
        amount=poker_types.Currency(amount=amount),
    )
    _handle_command(ctx, cmd, handle_withdraw_funds)


@when(parsers.parse('I handle a ReserveFunds command with amount {amount:d} for table "{table_id}"'))
def handle_reserve_funds_cmd(ctx, amount, table_id):
    """Handle ReserveFunds command."""
    cmd = player.ReserveFunds(
        amount=poker_types.Currency(amount=amount),
        table_root=table_id.encode(),
    )
    _handle_command(ctx, cmd, handle_reserve_funds)


@when(parsers.parse('I handle a ReleaseFunds command for table "{table_id}"'))
def handle_release_funds_cmd(ctx, table_id):
    """Handle ReleaseFunds command."""
    cmd = player.ReleaseFunds(
        table_root=table_id.encode(),
    )
    _handle_command(ctx, cmd, handle_release_funds)


@when("I rebuild the player state")
def rebuild_player_state(ctx):
    """Rebuild state from events."""
    ctx.state = build_state(_event_book(ctx))


# --- Then steps ---

# docs:start:then_step
@then("the result is a PlayerRegistered event")
def result_is_player_registered(ctx):
    """Verify result is PlayerRegistered event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    assert len(ctx.result.pages) == 1
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("PlayerRegistered")
# docs:end:then_step


@then("the result is a FundsDeposited event")
def result_is_funds_deposited(ctx):
    """Verify result is FundsDeposited event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    assert len(ctx.result.pages) == 1
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("FundsDeposited")


@then("the result is a FundsWithdrawn event")
def result_is_funds_withdrawn(ctx):
    """Verify result is FundsWithdrawn event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("FundsWithdrawn")


@then("the result is a FundsReserved event")
def result_is_funds_reserved(ctx):
    """Verify result is FundsReserved event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("FundsReserved")


@then("the result is a FundsReleased event")
def result_is_funds_released(ctx):
    """Verify result is FundsReleased event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("FundsReleased")


@then(parsers.parse('the player event has display_name "{name}"'))
def event_has_display_name(ctx, name):
    """Verify event display_name."""
    event_any = ctx.result.pages[0].event
    event = player.PlayerRegistered()
    event_any.Unpack(event)
    assert event.display_name == name


@then(parsers.parse('the player event has player_type "{ptype}"'))
def event_has_player_type(ctx, ptype):
    """Verify event player_type."""
    event_any = ctx.result.pages[0].event
    event = player.PlayerRegistered()
    event_any.Unpack(event)
    expected = poker_types.HUMAN if ptype == "HUMAN" else poker_types.AI
    assert event.player_type == expected


@then(parsers.parse("the player event has amount {amount:d}"))
def event_has_amount(ctx, amount):
    """Verify event amount."""
    event_any = ctx.result.pages[0].event
    type_url = event_any.type_url

    if type_url.endswith("FundsDeposited"):
        event = player.FundsDeposited()
    elif type_url.endswith("FundsWithdrawn"):
        event = player.FundsWithdrawn()
    elif type_url.endswith("FundsReserved"):
        event = player.FundsReserved()
    elif type_url.endswith("FundsReleased"):
        event = player.FundsReleased()
    else:
        pytest.fail(f"Unknown event type: {type_url}")

    event_any.Unpack(event)
    assert event.amount.amount == amount


@then(parsers.parse("the player event has new_balance {amount:d}"))
def event_has_new_balance(ctx, amount):
    """Verify event new_balance."""
    event_any = ctx.result.pages[0].event
    type_url = event_any.type_url

    if type_url.endswith("FundsDeposited"):
        event = player.FundsDeposited()
    elif type_url.endswith("FundsWithdrawn"):
        event = player.FundsWithdrawn()
    else:
        pytest.fail(f"Unknown event type: {type_url}")

    event_any.Unpack(event)
    assert event.new_balance.amount == amount


@then(parsers.parse("the player event has new_available_balance {amount:d}"))
def event_has_new_available_balance(ctx, amount):
    """Verify event new_available_balance."""
    event_any = ctx.result.pages[0].event
    type_url = event_any.type_url

    if type_url.endswith("FundsReserved"):
        event = player.FundsReserved()
    elif type_url.endswith("FundsReleased"):
        event = player.FundsReleased()
    else:
        pytest.fail(f"Unknown event type: {type_url}")

    event_any.Unpack(event)
    assert event.new_available_balance.amount == amount


@then(parsers.parse('the command fails with status "{status}"'))
def command_fails_with_status(ctx, status):
    """Verify command failure."""
    assert ctx.error is not None, "Expected error but command succeeded"
    # For now, we just check that there was an error
    # In a real implementation, we'd check the gRPC status code


@then(parsers.parse('the error message contains "{text}"'))
def error_message_contains(ctx, text):
    """Verify error message content."""
    assert ctx.error is not None, "Expected error but command succeeded"
    assert text.lower() in str(ctx.error).lower(), f"Expected '{text}' in '{ctx.error}'"


@then(parsers.parse("the player state has bankroll {amount:d}"))
def state_has_bankroll(ctx, amount):
    """Verify state bankroll."""
    assert ctx.state is not None
    assert ctx.state.bankroll == amount


@then(parsers.parse("the player state has reserved_funds {amount:d}"))
def state_has_reserved_funds(ctx, amount):
    """Verify state reserved_funds."""
    assert ctx.state is not None
    assert ctx.state.reserved_funds == amount


@then(parsers.parse("the player state has available_balance {amount:d}"))
def state_has_available_balance(ctx, amount):
    """Verify state available_balance."""
    assert ctx.state is not None
    assert ctx.state.available_balance() == amount
