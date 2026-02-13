"""Table aggregate unit tests."""

import sys
import importlib
import importlib.util
from pathlib import Path
from types import ModuleType

import pytest
from pytest_bdd import scenarios, given, when, then, parsers
from google.protobuf.any_pb2 import Any as ProtoAny

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import types_pb2 as poker_types
from angzarr_client.errors import CommandRejectedError


# Load table handlers as unique modules to avoid collision with other aggregates
def _load_handler_package(agg_path: Path, pkg_name: str) -> dict:
    """Load all handler modules under a unique package name."""
    handlers_path = agg_path / "handlers"

    # Create fake package for relative imports
    handlers_pkg = ModuleType(pkg_name)
    handlers_pkg.__path__ = [str(handlers_path)]
    handlers_pkg.__file__ = str(handlers_path / "__init__.py")
    sys.modules[pkg_name] = handlers_pkg

    modules = {}

    def load_module(module_name: str) -> ModuleType:
        file_path = handlers_path / f"{module_name}.py"
        full_name = f"{pkg_name}.{module_name}"

        spec = importlib.util.spec_from_file_location(
            full_name, file_path,
            submodule_search_locations=[str(handlers_path)]
        )
        module = importlib.util.module_from_spec(spec)
        module.__package__ = pkg_name
        sys.modules[full_name] = module
        spec.loader.exec_module(module)
        modules[module_name] = module
        return module

    # Load in dependency order (state first, then handlers that depend on it)
    load_module("state")
    load_module("create_table")
    load_module("join_table")
    load_module("leave_table")
    load_module("start_hand")
    load_module("end_hand")

    return modules


_agg_path = Path(__file__).parent.parent.parent / "agg-table"
_mods = _load_handler_package(_agg_path, "table_handlers")

TableState = _mods["state"].TableState
rebuild_state = _mods["state"].rebuild_state
handle_create_table = _mods["create_table"].handle_create_table
handle_join_table = _mods["join_table"].handle_join_table
handle_leave_table = _mods["leave_table"].handle_leave_table
handle_start_hand = _mods["start_hand"].handle_start_hand
handle_end_hand = _mods["end_hand"].handle_end_hand

# Add tests to path for conftest import
root = Path(__file__).parent.parent.parent
sys.path.insert(0, str(root))

from tests.conftest import (
    ScenarioContext, make_cover, make_command_book, make_timestamp, pack_event
)

# Load scenarios from feature file
scenarios("../../../features/unit/table.feature")


@pytest.fixture
def ctx():
    """Test context for table tests."""
    context = ScenarioContext()
    context.domain = "table"
    context.root = b"table-test"
    return context


def _event_book(ctx: ScenarioContext) -> types.EventBook:
    """Build EventBook from context."""
    return ctx.event_book()


def _handle_command(ctx: ScenarioContext, command_msg, handler_fn):
    """Execute a command handler."""
    cmd_any = ProtoAny()
    cmd_any.Pack(command_msg, type_url_prefix="type.poker/")

    event_book = _event_book(ctx)
    state = rebuild_state(event_book)
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

@given("no prior events for the table aggregate")
def no_prior_events(ctx):
    """Start with empty event history."""
    ctx.events = []


@given(parsers.parse('a TableCreated event for "{name}"'))
def table_created_event(ctx, name):
    """Add a TableCreated event."""
    event = table.TableCreated(
        table_name=name,
        game_variant=poker_types.TEXAS_HOLDEM,
        small_blind=5,
        big_blind=10,
        min_buy_in=200,
        max_buy_in=1000,
        max_players=9,
        action_timeout_seconds=30,
        created_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse('a TableCreated event for "{name}" with min_buy_in {min_buy_in:d}'))
def table_created_with_min_buy_in(ctx, name, min_buy_in):
    """Add a TableCreated event with specific min_buy_in."""
    event = table.TableCreated(
        table_name=name,
        game_variant=poker_types.TEXAS_HOLDEM,
        small_blind=5,
        big_blind=10,
        min_buy_in=min_buy_in,
        max_buy_in=1000,
        max_players=9,
        action_timeout_seconds=30,
        created_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse('a TableCreated event for "{name}" with max_players {max_players:d}'))
def table_created_with_max_players(ctx, name, max_players):
    """Add a TableCreated event with specific max_players."""
    event = table.TableCreated(
        table_name=name,
        game_variant=poker_types.TEXAS_HOLDEM,
        small_blind=5,
        big_blind=10,
        min_buy_in=200,
        max_buy_in=1000,
        max_players=max_players,
        action_timeout_seconds=30,
        created_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse('a PlayerJoined event for player "{player_id}" at seat {seat:d}'))
def player_joined_event(ctx, player_id, seat):
    """Add a PlayerJoined event."""
    event = table.PlayerJoined(
        player_root=player_id.encode(),
        seat_position=seat,
        buy_in_amount=500,
        stack=500,
        joined_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse('a PlayerJoined event for player "{player_id}" at seat {seat:d} with stack {stack:d}'))
def player_joined_with_stack(ctx, player_id, seat, stack):
    """Add a PlayerJoined event with specific stack."""
    event = table.PlayerJoined(
        player_root=player_id.encode(),
        seat_position=seat,
        buy_in_amount=stack,
        stack=stack,
        joined_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse("a HandStarted event for hand {hand_num:d}"))
def hand_started_event(ctx, hand_num):
    """Add a HandStarted event."""
    event = table.HandStarted(
        hand_root=f"hand-{hand_num}".encode(),
        hand_number=hand_num,
        dealer_position=0,
        started_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse("a HandStarted event for hand {hand_num:d} with dealer at seat {dealer:d}"))
def hand_started_with_dealer(ctx, hand_num, dealer):
    """Add a HandStarted event with specific dealer."""
    event = table.HandStarted(
        hand_root=f"hand-{hand_num}".encode(),
        hand_number=hand_num,
        dealer_position=dealer,
        started_at=make_timestamp(),
    )
    ctx.add_event(event)


@given(parsers.parse("a HandEnded event for hand {hand_num:d}"))
def hand_ended_event(ctx, hand_num):
    """Add a HandEnded event."""
    event = table.HandEnded(
        hand_root=f"hand-{hand_num}".encode(),
        ended_at=make_timestamp(),
    )
    ctx.add_event(event)


# --- When steps ---

@when(parsers.parse('I handle a CreateTable command with name "{name}" and variant "{variant}":'))
def handle_create_table_cmd(ctx, name, variant, datatable):
    """Handle CreateTable command with config from datatable."""
    # datatable is a list of lists: [headers, row1, row2...]
    headers = datatable[0]
    values = datatable[1]
    row = dict(zip(headers, values))

    variant_enum = poker_types.TEXAS_HOLDEM if variant == "TEXAS_HOLDEM" else poker_types.FIVE_CARD_DRAW

    cmd = table.CreateTable(
        table_name=name,
        game_variant=variant_enum,
        small_blind=int(row["small_blind"]),
        big_blind=int(row["big_blind"]),
        min_buy_in=int(row["min_buy_in"]),
        max_buy_in=int(row["max_buy_in"]),
        max_players=int(row["max_players"]),
        action_timeout_seconds=30,
    )
    _handle_command(ctx, cmd, handle_create_table)


@when(parsers.parse('I handle a JoinTable command for player "{player_id}" at seat {seat:d} with buy-in {buy_in:d}'))
def handle_join_table_cmd(ctx, player_id, seat, buy_in):
    """Handle JoinTable command."""
    cmd = table.JoinTable(
        player_root=player_id.encode(),
        preferred_seat=seat,
        buy_in_amount=buy_in,
    )
    _handle_command(ctx, cmd, handle_join_table)


@when(parsers.parse('I handle a LeaveTable command for player "{player_id}"'))
def handle_leave_table_cmd(ctx, player_id):
    """Handle LeaveTable command."""
    cmd = table.LeaveTable(
        player_root=player_id.encode(),
    )
    _handle_command(ctx, cmd, handle_leave_table)


@when("I handle a StartHand command")
def handle_start_hand_cmd(ctx):
    """Handle StartHand command."""
    cmd = table.StartHand()
    _handle_command(ctx, cmd, handle_start_hand)


@when(parsers.parse('I handle an EndHand command with winner "{winner_id}" winning {amount:d}'))
def handle_end_hand_cmd(ctx, winner_id, amount):
    """Handle EndHand command."""
    # Need hand root from state
    state = rebuild_state(_event_book(ctx))

    cmd = table.EndHand(
        hand_root=state.current_hand_root or b"",
        results=[
            table.PotResult(
                winner_root=winner_id.encode(),
                amount=amount,
            )
        ],
    )
    _handle_command(ctx, cmd, handle_end_hand)


@when("I rebuild the table state")
def rebuild_table_state(ctx):
    """Rebuild state from events."""
    ctx.state = rebuild_state(_event_book(ctx))


# --- Then steps ---

@then("the result is a TableCreated event")
def result_is_table_created(ctx):
    """Verify result is TableCreated event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    assert len(ctx.result.pages) == 1
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("TableCreated")


@then("the result is a PlayerJoined event")
def result_is_player_joined(ctx):
    """Verify result is PlayerJoined event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("PlayerJoined")


@then("the result is a PlayerLeft event")
def result_is_player_left(ctx):
    """Verify result is PlayerLeft event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("PlayerLeft")


@then("the result is a HandStarted event")
def result_is_hand_started(ctx):
    """Verify result is HandStarted event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("HandStarted")


@then("the result is a HandEnded event")
def result_is_hand_ended(ctx):
    """Verify result is HandEnded event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    assert event_any.type_url.endswith("HandEnded")


@then(parsers.parse('the table event has table_name "{name}"'))
def event_has_table_name(ctx, name):
    """Verify event table_name."""
    event_any = ctx.result.pages[0].event
    event = table.TableCreated()
    event_any.Unpack(event)
    assert event.table_name == name


@then(parsers.parse('the table event has game_variant "{variant}"'))
def event_has_game_variant(ctx, variant):
    """Verify event game_variant."""
    event_any = ctx.result.pages[0].event
    event = table.TableCreated()
    event_any.Unpack(event)
    expected = poker_types.TEXAS_HOLDEM if variant == "TEXAS_HOLDEM" else poker_types.FIVE_CARD_DRAW
    assert event.game_variant == expected


@then(parsers.parse("the table event has small_blind {amount:d}"))
def event_has_small_blind(ctx, amount):
    """Verify event small_blind."""
    event_any = ctx.result.pages[0].event
    event = table.TableCreated()
    event_any.Unpack(event)
    assert event.small_blind == amount


@then(parsers.parse("the table event has big_blind {amount:d}"))
def event_has_big_blind(ctx, amount):
    """Verify event big_blind."""
    event_any = ctx.result.pages[0].event
    event = table.TableCreated()
    event_any.Unpack(event)
    assert event.big_blind == amount


@then(parsers.parse("the table event has seat_position {seat:d}"))
def event_has_seat_position(ctx, seat):
    """Verify event seat_position."""
    event_any = ctx.result.pages[0].event
    event = table.PlayerJoined()
    event_any.Unpack(event)
    assert event.seat_position == seat


@then(parsers.parse("the table event has buy_in_amount {amount:d}"))
def event_has_buy_in_amount(ctx, amount):
    """Verify event buy_in_amount."""
    event_any = ctx.result.pages[0].event
    event = table.PlayerJoined()
    event_any.Unpack(event)
    assert event.buy_in_amount == amount


@then(parsers.parse("the table event has chips_cashed_out {amount:d}"))
def event_has_chips_cashed_out(ctx, amount):
    """Verify event chips_cashed_out."""
    event_any = ctx.result.pages[0].event
    event = table.PlayerLeft()
    event_any.Unpack(event)
    assert event.chips_cashed_out == amount


@then(parsers.parse("the table event has hand_number {num:d}"))
def event_has_hand_number(ctx, num):
    """Verify event hand_number."""
    event_any = ctx.result.pages[0].event
    event = table.HandStarted()
    event_any.Unpack(event)
    assert event.hand_number == num


@then(parsers.parse("the table event has {count:d} active_players"))
def event_has_active_players(ctx, count):
    """Verify event active_players count."""
    event_any = ctx.result.pages[0].event
    event = table.HandStarted()
    event_any.Unpack(event)
    assert len(event.active_players) == count


@then(parsers.parse("the table event has dealer_position {pos:d}"))
def event_has_dealer_position(ctx, pos):
    """Verify event dealer_position."""
    event_any = ctx.result.pages[0].event
    event = table.HandStarted()
    event_any.Unpack(event)
    assert event.dealer_position == pos


@then(parsers.parse('the command fails with status "{status}"'))
def command_fails_with_status(ctx, status):
    """Verify command failure."""
    assert ctx.error is not None, "Expected error but command succeeded"


@then(parsers.parse('the error message contains "{text}"'))
def error_message_contains(ctx, text):
    """Verify error message content."""
    assert ctx.error is not None, "Expected error but command succeeded"
    assert text.lower() in str(ctx.error).lower(), f"Expected '{text}' in '{ctx.error}'"


@then(parsers.parse("the table state has {count:d} players"))
def state_has_players(ctx, count):
    """Verify state player count."""
    assert ctx.state is not None
    assert ctx.state.player_count() == count


@then(parsers.parse('the table state has seat {seat:d} occupied by "{player_id}"'))
def state_has_seat_occupied(ctx, seat, player_id):
    """Verify seat occupancy."""
    assert ctx.state is not None
    seat_data = ctx.state.get_seat(seat)
    assert seat_data is not None, f"No seat at position {seat}"
    assert seat_data.player_root == player_id.encode()


@then(parsers.parse('the table state has status "{status}"'))
def state_has_status(ctx, status):
    """Verify state status."""
    assert ctx.state is not None
    assert ctx.state.status == status


@then(parsers.parse("the table state has hand_count {count:d}"))
def state_has_hand_count(ctx, count):
    """Verify state hand_count."""
    assert ctx.state is not None
    assert ctx.state.hand_count == count


@then(parsers.parse('player "{player_id}" stack change is {amount:d}'))
def player_stack_change(ctx, player_id, amount):
    """Verify player stack change in HandEnded event."""
    assert ctx.result is not None, f"Expected result, got error: {ctx.error}"
    event_any = ctx.result.pages[0].event
    event = table.HandEnded()
    event_any.Unpack(event)
    player_hex = player_id.encode().hex()
    assert player_hex in event.stack_changes, f"Player {player_id} not in stack_changes"
    assert event.stack_changes[player_hex] == amount
