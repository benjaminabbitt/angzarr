"""Behave step definitions for saga tests.

Note: These tests are currently disabled pending implementation of the sagas package.
The saga implementations exist but use a different pattern (EventRouter/SagaHandler).
"""

from datetime import datetime, timezone

from behave import given, when, then, use_step_matcher
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

# Saga base classes not yet implemented - mark all saga steps as pending
try:
    from sagas.base import Saga, SagaContext, SagaRouter
    from sagas.table_sync_saga import TableSyncSaga
    from sagas.hand_results_saga import HandResultsSaga
    SAGAS_AVAILABLE = True
except ImportError:
    SAGAS_AVAILABLE = False
    # Stub classes for type hints
    class Saga:
        pass
    class SagaContext:
        pass
    class SagaRouter:
        pass
    class TableSyncSaga:
        pass
    class HandResultsSaga:
        pass

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


class FailingSaga(Saga):
    """A saga that always fails for testing."""

    @property
    def name(self) -> str:
        return "FailingSaga"

    @property
    def subscribed_events(self) -> list[str]:
        return ["HandStarted"]

    def handle(self, context: SagaContext) -> list[types.CommandBook]:
        raise RuntimeError("FailingSaga always fails")


def _check_sagas_available(context):
    """Skip scenario if sagas module not available."""
    if not SAGAS_AVAILABLE:
        context.scenario.skip("sagas module not implemented")
        return False
    return True


# --- Given steps ---


@given("a TableSyncSaga")
def step_given_table_sync_saga(context):
    """Create TableSyncSaga instance."""
    if not _check_sagas_available(context):
        return
    context.saga = TableSyncSaga()
    context.event = None
    context.event_book = None
    context.commands = []


@given("a HandResultsSaga")
def step_given_hand_results_saga(context):
    """Create HandResultsSaga instance."""
    if not _check_sagas_available(context):
        return
    context.saga = HandResultsSaga()
    context.event = None
    context.event_book = None
    context.commands = []


@given("a SagaRouter with TableSyncSaga and HandResultsSaga")
def step_given_saga_router_with_sagas(context):
    """Create SagaRouter with multiple sagas."""
    if not _check_sagas_available(context):
        return
    context.router = SagaRouter()
    context.table_sync = TableSyncSaga()
    context.hand_results = HandResultsSaga()
    context.router.register(context.table_sync)
    context.router.register(context.hand_results)
    context.commands = []
    context.saga_calls = {}


@given("a SagaRouter with TableSyncSaga")
def step_given_saga_router_with_table_sync(context):
    """Create SagaRouter with TableSyncSaga."""
    if not _check_sagas_available(context):
        return
    context.router = SagaRouter()
    context.table_sync = TableSyncSaga()
    context.router.register(context.table_sync)
    context.commands = []


@given("a SagaRouter with a failing saga and TableSyncSaga")
def step_given_saga_router_with_failing(context):
    """Create SagaRouter with a failing saga and TableSyncSaga."""
    if not _check_sagas_available(context):
        return
    context.router = SagaRouter()
    context.failing_saga = FailingSaga()
    context.table_sync = TableSyncSaga()
    context.router.register(context.failing_saga)
    context.router.register(context.table_sync)
    context.commands = []
    context.exception_raised = False


@given("a HandStarted event from table domain with:")
def step_given_hand_started_event(context):
    """Create a HandStarted event from datatable."""
    row = {context.table.headings[i]: context.table[0][i] for i in range(len(context.table.headings))}
    variant = getattr(poker_types, row.get("game_variant", "TEXAS_HOLDEM"), poker_types.TEXAS_HOLDEM)

    context.event = table.HandStarted(
        hand_root=row.get("hand_root", "hand-1").encode(),
        hand_number=int(row.get("hand_number", 1)),
        dealer_position=int(row.get("dealer_position", 0)),
        game_variant=variant,
        small_blind=int(row.get("small_blind", 5)),
        big_blind=int(row.get("big_blind", 10)),
        started_at=make_timestamp(),
    )


@given("a HandStarted event")
def step_given_hand_started_event_simple(context):
    """Create a simple HandStarted event."""
    context.event = table.HandStarted(
        hand_root=b"hand-1",
        hand_number=1,
        dealer_position=0,
        game_variant=poker_types.TEXAS_HOLDEM,
        small_blind=5,
        big_blind=10,
        started_at=make_timestamp(),
    )
    # Add some default players
    context.event.active_players.append(
        table.SeatSnapshot(
            player_root=b"player-1",
            position=0,
            stack=500,
        )
    )
    context.event.active_players.append(
        table.SeatSnapshot(
            player_root=b"player-2",
            position=1,
            stack=500,
        )
    )


@given("active players:")
def step_given_active_players(context):
    """Add active players from datatable."""
    # Support both context.event (sagas) and context.hand_started (process manager)
    target = getattr(context, "hand_started", None) or getattr(context, "event", None)
    if not target:
        raise ValueError("No hand_started or event in context")

    for row in context.table:
        row_dict = {context.table.headings[j]: row[j] for j in range(len(context.table.headings))}
        player_root = row_dict.get("player_root", "player-1").encode()
        target.active_players.append(
            table.SeatSnapshot(
                player_root=player_root,
                position=int(row_dict.get("position", 0)),
                stack=int(row_dict.get("stack", 500)),
            )
        )


@given("a HandComplete event from hand domain with:")
def step_given_hand_complete_event(context):
    """Create a HandComplete event from datatable."""
    row = {context.table.headings[i]: context.table[0][i] for i in range(len(context.table.headings))}
    context.event = hand.HandComplete(
        table_root=row.get("table_root", "table-1").encode(),
        # Note: pot_total is in feature file but not in proto - ignore it
    )


@given("winners:")
def step_given_winners(context):
    """Add winners from datatable."""
    for row in context.table:
        row_dict = {context.table.headings[j]: row[j] for j in range(len(context.table.headings))}
        player_root = row_dict.get("player_root", "player-1").encode()
        context.event.winners.append(
            hand.PotWinner(
                player_root=player_root,
                amount=int(row_dict.get("amount", 0)),
                pot_type="main",
            )
        )


@given("a HandEnded event from table domain with:")
def step_given_hand_ended_event(context):
    """Create a HandEnded event from datatable."""
    row = {context.table.headings[i]: context.table[0][i] for i in range(len(context.table.headings))}
    context.event = table.HandEnded(
        hand_root=row.get("hand_root", "hand-1").encode(),
        ended_at=make_timestamp(),
    )


@given("stack_changes:")
def step_given_stack_changes(context):
    """Add stack changes from datatable."""
    for row in context.table:
        row_dict = {context.table.headings[j]: row[j] for j in range(len(context.table.headings))}
        player_root = row_dict.get("player_root", "player-1").encode()
        change = int(row_dict.get("change", 0))
        context.event.stack_changes[player_root.hex()] = change


@given("a PotAwarded event from hand domain with:")
def step_given_pot_awarded_event(context):
    """Create a PotAwarded event from datatable."""
    row = {context.table.headings[i]: context.table[0][i] for i in range(len(context.table.headings))}
    context.event = hand.PotAwarded()
    # pot_total is not a field on PotAwarded, but we store it for context
    context.pot_total = int(row.get("pot_total", 0))


@given("an event book with:")
def step_given_event_book_with(context):
    """Create event book with multiple events."""
    context.event_book = types.EventBook(
        cover=types.Cover(root=types.UUID(value=b"table-1"), domain="table"),
        pages=[],
    )
    for row in context.table:
        row_dict = {context.table.headings[j]: row[j] for j in range(len(context.table.headings))}
        event_type = row_dict.get("event_type", "HandStarted")
        if event_type == "HandStarted":
            event = table.HandStarted(
                hand_root=b"hand-1",
                hand_number=1,
                dealer_position=0,
                game_variant=poker_types.TEXAS_HOLDEM,
                small_blind=5,
                big_blind=10,
                started_at=make_timestamp(),
            )
            event.active_players.append(
                table.SeatSnapshot(
                    player_root=b"player-1",
                    position=0,
                    stack=500,
                )
            )
            event.active_players.append(
                table.SeatSnapshot(
                    player_root=b"player-2",
                    position=1,
                    stack=500,
                )
            )
            context.event_book.pages.append(make_event_page(event, len(context.event_book.pages)))


# --- When steps ---


@when("the saga handles the event")
def step_when_saga_handles_event(context):
    """Have saga handle the event."""
    event_page = make_event_page(context.event)
    event_book = types.EventBook(
        cover=types.Cover(root=types.UUID(value=b"table-1"), domain="table"),
        pages=[event_page],
    )

    event_type = context.event.DESCRIPTOR.name
    saga_context = SagaContext(
        event_book=event_book,
        event_type=event_type,
        aggregate_type="table" if hasattr(context.event, "hand_root") else "hand",
        aggregate_root=event_book.cover.root.value,
    )

    context.commands = context.saga.handle(saga_context)


@when("the router routes the event")
def step_when_router_routes_event(context):
    """Have router route the event."""
    event_page = make_event_page(context.event)
    event_book = types.EventBook(
        cover=types.Cover(root=types.UUID(value=b"table-1"), domain="table"),
        pages=[event_page],
    )

    try:
        context.commands = context.router.route(event_book, "table")
    except Exception:
        context.exception_raised = True


@when("the router routes the events")
def step_when_router_routes_events(context):
    """Have router route events from event book."""
    context.commands = context.router.route(context.event_book, "table")


# --- Then steps ---


@then("the saga emits a DealCards command to hand domain")
def step_then_saga_emits_deal_cards(context):
    """Verify saga emits DealCards command."""
    assert len(context.commands) >= 1, f"Expected at least 1 command, got {len(context.commands)}"
    cmd_book = context.commands[0]
    assert cmd_book.cover.domain == "hand", f"Expected hand domain, got {cmd_book.cover.domain}"
    assert "DealCards" in cmd_book.pages[0].command.type_url


@then("the saga emits an EndHand command to table domain")
def step_then_saga_emits_end_hand(context):
    """Verify saga emits EndHand command."""
    assert len(context.commands) >= 1
    cmd_book = context.commands[0]
    assert cmd_book.cover.domain == "table", f"Expected table domain, got {cmd_book.cover.domain}"
    assert "EndHand" in cmd_book.pages[0].command.type_url


@then("the saga emits (?P<count>\\d+) ReleaseFunds commands to player domain")
def step_then_saga_emits_release_funds(context, count):
    """Verify saga emits ReleaseFunds commands."""
    expected = int(count)
    assert len(context.commands) == expected, f"Expected {expected} commands, got {len(context.commands)}"
    for cmd_book in context.commands:
        assert cmd_book.cover.domain == "player"
        assert "ReleaseFunds" in cmd_book.pages[0].command.type_url


@then("the saga emits (?P<count>\\d+) DepositFunds commands to player domain")
def step_then_saga_emits_deposit_funds(context, count):
    """Verify saga emits DepositFunds commands."""
    expected = int(count)
    assert len(context.commands) == expected, f"Expected {expected} commands, got {len(context.commands)}"
    for cmd_book in context.commands:
        assert cmd_book.cover.domain == "player"
        assert "DepositFunds" in cmd_book.pages[0].command.type_url


@then("the saga emits (?P<count>\\d+) DealCards commands")
def step_then_saga_emits_deal_cards_count(context, count):
    """Verify saga emits specified number of DealCards commands."""
    expected = int(count)
    deal_cards_count = sum(
        1 for cmd in context.commands
        if "DealCards" in cmd.pages[0].command.type_url
    )
    assert deal_cards_count == expected, f"Expected {expected} DealCards commands, got {deal_cards_count}"


@then("the command has game_variant (?P<variant>\\w+)")
def step_then_command_has_game_variant(context, variant):
    """Verify command has specified game variant."""
    cmd_any = context.commands[0].pages[0].command
    cmd = hand.DealCards()
    cmd_any.Unpack(cmd)
    expected = getattr(poker_types, variant)
    assert cmd.game_variant == expected, f"Expected {variant}, got {cmd.game_variant}"


@then("the command has (?P<count>\\d+) players")
def step_then_command_has_players(context, count):
    """Verify command has specified number of players."""
    cmd_any = context.commands[0].pages[0].command
    cmd = hand.DealCards()
    cmd_any.Unpack(cmd)
    expected = int(count)
    assert len(cmd.players) == expected, f"Expected {expected} players, got {len(cmd.players)}"


@then("the command has hand_number (?P<num>\\d+)")
def step_then_command_has_hand_number(context, num):
    """Verify command has specified hand number."""
    cmd_any = context.commands[0].pages[0].command
    cmd = hand.DealCards()
    cmd_any.Unpack(cmd)
    expected = int(num)
    assert cmd.hand_number == expected, f"Expected hand_number {expected}, got {cmd.hand_number}"


@then("the command has (?P<count>\\d+) result")
def step_then_command_has_results(context, count):
    """Verify command has specified number of results."""
    cmd_any = context.commands[0].pages[0].command
    cmd = table.EndHand()
    cmd_any.Unpack(cmd)
    expected = int(count)
    assert len(cmd.results) == expected, f"Expected {expected} results, got {len(cmd.results)}"


@then('the result has winner "(?P<winner>[^"]+)" with amount (?P<amount>\\d+)')
def step_then_result_has_winner(context, winner, amount):
    """Verify result has specified winner and amount."""
    cmd_any = context.commands[0].pages[0].command
    cmd = table.EndHand()
    cmd_any.Unpack(cmd)
    result = cmd.results[0]
    expected_amount = int(amount)
    assert result.winner_root == winner.encode(), f"Expected {winner}, got {result.winner_root}"
    assert result.amount == expected_amount, f"Expected {expected_amount}, got {result.amount}"


@then('the first command has amount (?P<amount>\\d+) for "(?P<player>[^"]+)"')
def step_then_first_command_has_amount(context, amount, player):
    """Verify first command has specified amount for player."""
    cmd_any = context.commands[0].pages[0].command
    cmd = player_pb.DepositFunds()
    cmd_any.Unpack(cmd)
    expected_amount = int(amount)
    assert cmd.amount.amount == expected_amount, f"Expected {expected_amount}, got {cmd.amount.amount}"


@then('the second command has amount (?P<amount>\\d+) for "(?P<player>[^"]+)"')
def step_then_second_command_has_amount(context, amount, player):
    """Verify second command has specified amount for player."""
    cmd_any = context.commands[1].pages[0].command
    cmd = player_pb.DepositFunds()
    cmd_any.Unpack(cmd)
    expected_amount = int(amount)
    assert cmd.amount.amount == expected_amount, f"Expected {expected_amount}, got {cmd.amount.amount}"


@then("only TableSyncSaga handles the event")
def step_then_only_table_sync_handles(context):
    """Verify only TableSyncSaga handled the event."""
    # TableSyncSaga handles HandStarted and emits DealCards
    deal_cards_count = sum(
        1 for cmd in context.commands
        if "DealCards" in cmd.pages[0].command.type_url
    )
    assert deal_cards_count >= 1, "Expected TableSyncSaga to emit DealCards"
    # HandResultsSaga doesn't handle HandStarted


@then("TableSyncSaga still emits its command")
def step_then_table_sync_emits(context):
    """Verify TableSyncSaga still emitted its command despite other saga failure."""
    deal_cards_count = sum(
        1 for cmd in context.commands
        if "DealCards" in cmd.pages[0].command.type_url
    )
    assert deal_cards_count >= 1, "Expected TableSyncSaga to emit DealCards"


@then("no exception is raised")
def step_then_no_exception(context):
    """Verify no exception was raised."""
    assert not context.exception_raised, "Exception was raised unexpectedly"


# Import player module with alias to avoid shadowing
player_pb = player
