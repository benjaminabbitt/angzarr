"""Step definitions for table aggregate tests."""

import importlib.util
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
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import types_pb2 as poker_types
from angzarr_client.errors import CommandRejectedError

# Import table handlers explicitly from the correct path
_table_handlers_path = project_root / "agg-table" / "handlers"
_table_handlers_init = _table_handlers_path / "__init__.py"
_spec = importlib.util.spec_from_file_location("table_handlers", _table_handlers_init)
_table_handlers = importlib.util.module_from_spec(_spec)
sys.modules["table_handlers"] = _table_handlers
_spec.loader.exec_module(_table_handlers)

handle_create_table = _table_handlers.handle_create_table
handle_join_table = _table_handlers.handle_join_table
handle_leave_table = _table_handlers.handle_leave_table
handle_start_hand = _table_handlers.handle_start_hand
handle_end_hand = _table_handlers.handle_end_hand
build_state = _table_handlers.build_state
TableState = _table_handlers.TableState


def state_from_event_book(event_book):
    """Build state from EventBook - extracts Any-wrapped events and applies them."""
    state = TableState()
    if event_book is None:
        return state
    events = [page.event for page in event_book.pages if page.event]
    return build_state(state, events)
TableState = _table_handlers.TableState

# Use regex matchers for flexibility
use_step_matcher("re")


def make_timestamp():
    """Create current timestamp."""
    return Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))


def make_event_page(event_msg, num: int = 0) -> types.EventPage:
    """Create EventPage with packed event."""
    event_any = ProtoAny()
    event_any.Pack(event_msg, type_url_prefix="type.googleapis.com/")
    return types.EventPage(
        num=num,
        event=event_any,
        created_at=make_timestamp(),
    )


def _make_event_book(pages):
    """Create an EventBook from a list of EventPages."""
    return types.EventBook(
        cover=types.Cover(
            root=types.UUID(value=b"table-123"),
            domain="table",
        ),
        pages=pages,
    )


def _make_command_book(command_msg):
    """Create a CommandBook with a packed command."""
    command_any = ProtoAny()
    command_any.Pack(command_msg, type_url_prefix="type.googleapis.com/")
    return types.CommandBook(
        cover=types.Cover(
            root=types.UUID(value=b"table-123"),
            domain="table",
        ),
        pages=[
            types.CommandPage(
                sequence=0,
                command=command_any,
            )
        ],
    )


# --- Given steps ---


@given(r"no prior events for the table aggregate")
def step_given_no_prior_events(context):
    """Initialize with empty event history."""
    context.events = []
    context.state = TableState()


@given(r'a TableCreated event for "(?P<name>[^"]+)"')
def step_given_table_created(context, name):
    """Add a TableCreated event to history."""
    if not hasattr(context, "events"):
        context.events = []

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
    context.events.append(make_event_page(event, num=len(context.events)))


@given(r'a TableCreated event for "(?P<name>[^"]+)" with min_buy_in (?P<min_buy_in>\d+)')
def step_given_table_created_min_buy_in(context, name, min_buy_in):
    """Add a TableCreated event with specific min_buy_in."""
    if not hasattr(context, "events"):
        context.events = []

    event = table.TableCreated(
        table_name=name,
        game_variant=poker_types.TEXAS_HOLDEM,
        small_blind=5,
        big_blind=10,
        min_buy_in=int(min_buy_in),
        max_buy_in=1000,
        max_players=9,
        action_timeout_seconds=30,
        created_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, num=len(context.events)))


@given(r'a TableCreated event for "(?P<name>[^"]+)" with max_players (?P<max_players>\d+)')
def step_given_table_created_max_players(context, name, max_players):
    """Add a TableCreated event with specific max_players."""
    if not hasattr(context, "events"):
        context.events = []

    event = table.TableCreated(
        table_name=name,
        game_variant=poker_types.TEXAS_HOLDEM,
        small_blind=5,
        big_blind=10,
        min_buy_in=200,
        max_buy_in=1000,
        max_players=int(max_players),
        action_timeout_seconds=30,
        created_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, num=len(context.events)))


@given(r'a PlayerJoined event for player "(?P<player_id>[^"]+)" at seat (?P<seat>\d+)')
def step_given_player_joined(context, player_id, seat):
    """Add a PlayerJoined event with default stack."""
    if not hasattr(context, "events"):
        context.events = []

    event = table.PlayerJoined(
        player_root=player_id.encode("utf-8"),
        seat_position=int(seat),
        buy_in_amount=500,
        stack=500,
        joined_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, num=len(context.events)))


@given(
    r'a PlayerJoined event for player "(?P<player_id>[^"]+)" at seat (?P<seat>\d+) with stack (?P<stack>\d+)'
)
def step_given_player_joined_with_stack(context, player_id, seat, stack):
    """Add a PlayerJoined event with specific stack."""
    if not hasattr(context, "events"):
        context.events = []

    event = table.PlayerJoined(
        player_root=player_id.encode("utf-8"),
        seat_position=int(seat),
        buy_in_amount=int(stack),
        stack=int(stack),
        joined_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, num=len(context.events)))


@given(r"a HandStarted event for hand (?P<hand_num>\d+)")
def step_given_hand_started(context, hand_num):
    """Add a HandStarted event."""
    if not hasattr(context, "events"):
        context.events = []

    # Get seated players from prior events to build hand root
    event_book = _make_event_book(context.events)
    state = state_from_event_book(event_book)

    active_players = []
    for pos, seat in state.seats.items():
        active_players.append(
            table.SeatSnapshot(
                position=pos,
                player_root=seat.player_root,
                stack=seat.stack,
            )
        )

    event = table.HandStarted(
        hand_root=f"hand-{hand_num}".encode(),
        hand_number=int(hand_num),
        dealer_position=0,
        small_blind_position=0,
        big_blind_position=1,
        game_variant=state.game_variant if state.exists() else poker_types.TEXAS_HOLDEM,
        small_blind=state.small_blind if state.exists() else 5,
        big_blind=state.big_blind if state.exists() else 10,
        started_at=make_timestamp(),
    )
    event.active_players.extend(active_players)
    context.events.append(make_event_page(event, num=len(context.events)))


@given(r"a HandStarted event for hand (?P<hand_num>\d+) with dealer at seat (?P<seat>\d+)")
def step_given_hand_started_with_dealer(context, hand_num, seat):
    """Add a HandStarted event with specific dealer position."""
    if not hasattr(context, "events"):
        context.events = []

    # Get seated players from prior events
    event_book = _make_event_book(context.events)
    state = state_from_event_book(event_book)

    active_players = []
    for pos, seat_state in state.seats.items():
        active_players.append(
            table.SeatSnapshot(
                position=pos,
                player_root=seat_state.player_root,
                stack=seat_state.stack,
            )
        )

    event = table.HandStarted(
        hand_root=f"hand-{hand_num}".encode(),
        hand_number=int(hand_num),
        dealer_position=int(seat),
        small_blind_position=int(seat),
        big_blind_position=1,
        game_variant=state.game_variant if state.exists() else poker_types.TEXAS_HOLDEM,
        small_blind=state.small_blind if state.exists() else 5,
        big_blind=state.big_blind if state.exists() else 10,
        started_at=make_timestamp(),
    )
    event.active_players.extend(active_players)
    context.events.append(make_event_page(event, num=len(context.events)))


@given(r"a HandEnded event for hand (?P<hand_num>\d+)")
def step_given_hand_ended(context, hand_num):
    """Add a HandEnded event."""
    if not hasattr(context, "events"):
        context.events = []

    event = table.HandEnded(
        hand_root=f"hand-{hand_num}".encode(),
        ended_at=make_timestamp(),
    )
    context.events.append(make_event_page(event, num=len(context.events)))


# --- When steps ---


@when(
    r'I handle a CreateTable command with name "(?P<name>[^"]+)" and variant "(?P<variant>[^"]+)":'
)
def step_when_create_table(context, name, variant):
    """Handle CreateTable command with datatable."""
    row = {
        context.table.headings[i]: context.table[0][i]
        for i in range(len(context.table.headings))
    }
    game_variant = getattr(poker_types, variant, poker_types.TEXAS_HOLDEM)

    cmd = table.CreateTable(
        table_name=name,
        game_variant=game_variant,
        small_blind=int(row.get("small_blind", 5)),
        big_blind=int(row.get("big_blind", 10)),
        min_buy_in=int(row.get("min_buy_in", 200)),
        max_buy_in=int(row.get("max_buy_in", 1000)),
        max_players=int(row.get("max_players", 9)),
        action_timeout_seconds=int(row.get("action_timeout_seconds", 30)),
    )
    _execute_handler(context, cmd, handle_create_table)


@when(
    r'I handle a JoinTable command for player "(?P<player_id>[^"]+)" at seat (?P<seat>-?\d+) with buy-in (?P<buy_in>\d+)'
)
def step_when_join_table(context, player_id, seat, buy_in):
    """Handle JoinTable command."""
    cmd = table.JoinTable(
        player_root=player_id.encode("utf-8"),
        preferred_seat=int(seat),
        buy_in_amount=int(buy_in),
    )
    _execute_handler(context, cmd, handle_join_table)


@when(r'I handle a LeaveTable command for player "(?P<player_id>[^"]+)"')
def step_when_leave_table(context, player_id):
    """Handle LeaveTable command."""
    cmd = table.LeaveTable(
        player_root=player_id.encode("utf-8"),
    )
    _execute_handler(context, cmd, handle_leave_table)


@when(r"I handle a StartHand command")
def step_when_start_hand(context):
    """Handle StartHand command."""
    cmd = table.StartHand()
    _execute_handler(context, cmd, handle_start_hand)


@when(r'I handle an EndHand command with winner "(?P<winner>[^"]+)" winning (?P<amount>\d+)')
def step_when_end_hand(context, winner, amount):
    """Handle EndHand command."""
    # Get current hand root from state
    event_book = _make_event_book(context.events if hasattr(context, "events") else [])
    state = state_from_event_book(event_book)

    cmd = table.EndHand(
        hand_root=state.current_hand_root,
    )
    cmd.results.append(
        table.PotResult(
            winner_root=winner.encode("utf-8"),
            amount=int(amount),
            pot_type="main",
        )
    )
    _execute_handler(context, cmd, handle_end_hand)


@when(r"I rebuild the table state")
def step_when_rebuild_state(context):
    """Rebuild table state from events."""
    event_book = _make_event_book(context.events)
    context.state = state_from_event_book(event_book)


def _execute_handler(context, cmd, handler):
    """Execute a command handler and capture result or error."""
    event_book = _make_event_book(context.events if hasattr(context, "events") else [])
    state = state_from_event_book(event_book)
    command_book = _make_command_book(cmd)
    seq = len(context.events) if hasattr(context, "events") else 0

    try:
        result = handler(command_book, command_book.pages[0].command, state, seq)
        context.result = result
        context.error = None
        # Extract the event for assertion steps
        if result.pages:
            context.result_event_any = result.pages[0].event
    except CommandRejectedError as e:
        context.result = None
        context.error = e
        context.error_message = str(e)


# --- Then steps ---


@then(r"the result is a (?P<event_type>\w+) event")
def step_then_result_is_event(context, event_type):
    """Verify the result event type."""
    assert (
        context.result is not None
    ), f"Expected {event_type} event but got error: {context.error}"
    assert context.result.pages, "No event pages in result"
    event_any = context.result.pages[0].event
    assert event_any.type_url.endswith(
        event_type
    ), f"Expected {event_type} but got {event_any.type_url}"


@then(r'the table event has table_name "(?P<name>[^"]+)"')
def step_then_event_has_table_name(context, name):
    """Verify the event table_name field."""
    event = table.TableCreated()
    context.result_event_any.Unpack(event)
    assert event.table_name == name, f"Expected table_name={name}, got {event.table_name}"


@then(r'the table event has game_variant "(?P<variant>[^"]+)"')
def step_then_event_has_game_variant(context, variant):
    """Verify the event game_variant field."""
    event = table.TableCreated()
    context.result_event_any.Unpack(event)
    expected = getattr(poker_types, variant)
    assert (
        event.game_variant == expected
    ), f"Expected game_variant={variant}, got {event.game_variant}"


@then(r"the table event has small_blind (?P<amount>\d+)")
def step_then_event_has_small_blind(context, amount):
    """Verify the event small_blind field."""
    event = table.TableCreated()
    context.result_event_any.Unpack(event)
    assert (
        event.small_blind == int(amount)
    ), f"Expected small_blind={amount}, got {event.small_blind}"


@then(r"the table event has big_blind (?P<amount>\d+)")
def step_then_event_has_big_blind(context, amount):
    """Verify the event big_blind field."""
    event = table.TableCreated()
    context.result_event_any.Unpack(event)
    assert (
        event.big_blind == int(amount)
    ), f"Expected big_blind={amount}, got {event.big_blind}"


@then(r"the table event has seat_position (?P<pos>\d+)")
def step_then_event_has_seat_position(context, pos):
    """Verify the event seat_position field."""
    event = table.PlayerJoined()
    context.result_event_any.Unpack(event)
    assert (
        event.seat_position == int(pos)
    ), f"Expected seat_position={pos}, got {event.seat_position}"


@then(r"the table event has buy_in_amount (?P<amount>\d+)")
def step_then_event_has_buy_in_amount(context, amount):
    """Verify the event buy_in_amount field."""
    event = table.PlayerJoined()
    context.result_event_any.Unpack(event)
    assert (
        event.buy_in_amount == int(amount)
    ), f"Expected buy_in_amount={amount}, got {event.buy_in_amount}"


@then(r"the table event has chips_cashed_out (?P<amount>\d+)")
def step_then_event_has_chips_cashed_out(context, amount):
    """Verify the event chips_cashed_out field."""
    event = table.PlayerLeft()
    context.result_event_any.Unpack(event)
    assert (
        event.chips_cashed_out == int(amount)
    ), f"Expected chips_cashed_out={amount}, got {event.chips_cashed_out}"


@then(r"the table event has hand_number (?P<num>\d+)")
def step_then_event_has_hand_number(context, num):
    """Verify the event hand_number field."""
    event = table.HandStarted()
    context.result_event_any.Unpack(event)
    assert (
        event.hand_number == int(num)
    ), f"Expected hand_number={num}, got {event.hand_number}"


@then(r"the table event has (?P<count>\d+) active_players")
def step_then_event_has_active_players(context, count):
    """Verify the event active_players count."""
    event = table.HandStarted()
    context.result_event_any.Unpack(event)
    assert (
        len(event.active_players) == int(count)
    ), f"Expected {count} active_players, got {len(event.active_players)}"


@then(r"the table event has dealer_position (?P<pos>\d+)")
def step_then_event_has_dealer_position(context, pos):
    """Verify the event dealer_position field."""
    event = table.HandStarted()
    context.result_event_any.Unpack(event)
    assert (
        event.dealer_position == int(pos)
    ), f"Expected dealer_position={pos}, got {event.dealer_position}"


@then(r'player "(?P<player_id>[^"]+)" stack change is (?P<amount>-?\d+)')
def step_then_player_stack_change(context, player_id, amount):
    """Verify the player's stack change in HandEnded event."""
    event = table.HandEnded()
    context.result_event_any.Unpack(event)
    player_hex = player_id.encode("utf-8").hex()
    assert player_hex in event.stack_changes, f"No stack change for {player_id}"
    assert (
        event.stack_changes[player_hex] == int(amount)
    ), f"Expected stack change {amount}, got {event.stack_changes[player_hex]}"


@then(r'the command fails with status "(?P<status>[^"]+)"')
def step_then_command_fails(context, status):
    """Verify the command failed."""
    assert context.error is not None, "Expected command to fail but it succeeded"


@then(r'the error message contains "(?P<text>[^"]+)"')
def step_then_error_contains(context, text):
    """Verify the error message contains expected text."""
    assert context.error is not None, "Expected an error but got success"
    assert (
        text.lower() in context.error_message.lower()
    ), f"Expected error to contain '{text}', got '{context.error_message}'"


@then(r"the table state has (?P<count>\d+) players")
def step_then_state_has_players(context, count):
    """Verify the table state player count."""
    assert context.state is not None, "No table state"
    assert (
        context.state.player_count() == int(count)
    ), f"Expected {count} players, got {context.state.player_count()}"


@then(r'the table state has seat (?P<seat>\d+) occupied by "(?P<player_id>[^"]+)"')
def step_then_state_seat_occupied(context, seat, player_id):
    """Verify the table state seat occupancy."""
    assert context.state is not None, "No table state"
    seat_state = context.state.get_seat(int(seat))
    assert seat_state is not None, f"Seat {seat} not occupied"
    expected_player = player_id.encode("utf-8")
    assert (
        seat_state.player_root == expected_player
    ), f"Expected {player_id} at seat {seat}, got {seat_state.player_root}"


@then(r'the table state has status "(?P<status>[^"]+)"')
def step_then_state_has_status(context, status):
    """Verify the table state status."""
    assert context.state is not None, "No table state"
    assert (
        context.state.status == status
    ), f"Expected status={status}, got {context.state.status}"


@then(r"the table state has hand_count (?P<count>\d+)")
def step_then_state_has_hand_count(context, count):
    """Verify the table state hand_count."""
    assert context.state is not None, "No table state"
    assert (
        context.state.hand_count == int(count)
    ), f"Expected hand_count={count}, got {context.state.hand_count}"
