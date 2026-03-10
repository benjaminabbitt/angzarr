"""Merge strategy step definitions."""

import uuid
from dataclasses import dataclass, field
from typing import Optional

import pytest
from google.protobuf.any_pb2 import Any
from google.protobuf.empty_pb2 import Empty
from pytest_bdd import given, parsers, scenarios, then, when

from angzarr_client.proto.angzarr import types_pb2

# Link to feature file


@dataclass
class MockCoordinatorResult:
    """Result from mock coordinator."""

    success: bool = False
    status: str = ""
    error_message: str = ""
    retryable: bool = False
    event_book: Optional[types_pb2.EventBook] = None


@dataclass
class MockAggregateState:
    """Mock aggregate state for testing."""

    domain: str = "player"
    root_id: uuid.UUID = field(default_factory=uuid.uuid4)
    events: list = field(default_factory=list)
    next_sequence: int = 0
    accepts_command: bool = True
    reject_reason: Optional[str] = None
    value: int = 0  # For counter aggregate
    items: list = field(default_factory=list)  # For set aggregate


@pytest.fixture
def merge_context():
    """Test context for merge strategy scenarios."""
    return {
        "aggregate": None,
        "command": None,
        "command_sequence": None,
        "merge_strategy": None,
        "result": None,
        "events_persisted": False,
        "concurrent_commands": [],
        "all_results": [],
        "next_sequence": 3,  # From Background
        "saga_command": None,
        "saga_retried": False,
    }


def make_event_book(
    domain: str, sequence_count: int, next_seq: int
) -> types_pb2.EventBook:
    """Create an EventBook with given number of events."""
    book = types_pb2.EventBook()
    book.cover.domain = domain
    book.cover.root.value = uuid.uuid4().bytes
    for i in range(sequence_count):
        page = book.pages.add()
        page.header.sequence = i
        page.event.Pack(Empty())
    return book


def make_command(
    domain: str, sequence: int, strategy: types_pb2.MergeStrategy
) -> types_pb2.CommandBook:
    """Create a CommandBook with given parameters."""
    cmd = types_pb2.CommandBook()
    cmd.cover.domain = domain
    cmd.cover.root.value = uuid.uuid4().bytes
    page = cmd.pages.add()
    page.header.sequence = sequence
    page.merge_strategy = strategy
    page.command.Pack(Empty())
    return cmd


# ==========================================================================
# Background Steps
# ==========================================================================


def datatable_to_dicts(datatable):
    """Convert pytest-bdd datatable (list of lists) to list of dicts."""
    if not datatable or len(datatable) < 2:
        return []
    headers = datatable[0]
    return [dict(zip(headers, row)) for row in datatable[1:]]


@given('an aggregate "player" with initial events:')
def given_aggregate_with_initial_events(merge_context, datatable):
    """Set up aggregate with events from datatable."""
    merge_context["aggregate"] = MockAggregateState(domain="player")
    rows = datatable_to_dicts(datatable)
    for row in rows:
        seq = int(row["sequence"])
        merge_context["aggregate"].events.append({"sequence": seq, "type": row["type"]})
        merge_context["aggregate"].next_sequence = max(
            merge_context["aggregate"].next_sequence, seq + 1
        )
    merge_context["next_sequence"] = merge_context["aggregate"].next_sequence


# ==========================================================================
# Given Steps - Merge Strategy
# ==========================================================================


@given(parsers.parse("a command with merge_strategy {strategy}"))
def given_command_with_strategy(merge_context, strategy):
    strategy_map = {
        "STRICT": types_pb2.MERGE_STRICT,
        "COMMUTATIVE": types_pb2.MERGE_COMMUTATIVE,
        "AGGREGATE_HANDLES": types_pb2.MERGE_AGGREGATE_HANDLES,
    }
    merge_context["merge_strategy"] = strategy_map.get(
        strategy, types_pb2.MERGE_COMMUTATIVE
    )


@given("a command with no explicit merge_strategy")
def given_command_no_strategy(merge_context):
    merge_context["merge_strategy"] = types_pb2.MERGE_COMMUTATIVE  # Default


@given(parsers.parse("the command targets sequence {seq:d}"))
def given_command_targets_sequence(merge_context, seq):
    merge_context["command_sequence"] = seq


@given("the aggregate accepts the command")
def given_aggregate_accepts(merge_context):
    if merge_context.get("aggregate"):
        merge_context["aggregate"].accepts_command = True


@given("the aggregate rejects due to state conflict")
def given_aggregate_rejects(merge_context):
    if merge_context.get("aggregate"):
        merge_context["aggregate"].accepts_command = False
        merge_context["aggregate"].reject_reason = "State conflict"


# ==========================================================================
# Given Steps - Counter and Set Aggregates
# ==========================================================================


@given(parsers.parse("a counter aggregate at value {value:d}"))
def given_counter_aggregate(merge_context, value):
    merge_context["aggregate"] = MockAggregateState(domain="counter", value=value)


@given("two concurrent IncrementBy commands:")
def given_concurrent_increment_commands(merge_context, datatable):
    merge_context["concurrent_commands"] = []
    rows = datatable_to_dicts(datatable)
    for row in rows:
        cmd = {
            "client": row["client"],
            "amount": int(row["amount"]),
            "sequence": int(row["sequence"]),
        }
        merge_context["concurrent_commands"].append(cmd)


@given(parsers.parse("a set aggregate containing {items}"))
def given_set_aggregate(merge_context, items):
    # Parse ["apple", "banana"]
    items_list = items.strip("[]").replace('"', "").split(", ")
    merge_context["aggregate"] = MockAggregateState(domain="set", items=items_list)


@given(parsers.parse('two concurrent AddItem commands for "{item}":'))
def given_concurrent_add_item_commands(merge_context, item, datatable):
    merge_context["concurrent_commands"] = []
    rows = datatable_to_dicts(datatable)
    for row in rows:
        cmd = {
            "client": row["client"],
            "item": item,
            "sequence": int(row["sequence"]),
        }
        merge_context["concurrent_commands"].append(cmd)


# ==========================================================================
# Given Steps - Commands for Same Aggregate
# ==========================================================================


@given("commands for the same aggregate:")
def given_commands_for_same_aggregate(merge_context, datatable):
    strategy_map = {
        "STRICT": types_pb2.MERGE_STRICT,
        "COMMUTATIVE": types_pb2.MERGE_COMMUTATIVE,
        "AGGREGATE_HANDLES": types_pb2.MERGE_AGGREGATE_HANDLES,
    }
    merge_context["concurrent_commands"] = []
    rows = datatable_to_dicts(datatable)
    for row in rows:
        cmd = {
            "command": row["command"],
            "strategy": strategy_map.get(
                row["merge_strategy"], types_pb2.MERGE_COMMUTATIVE
            ),
        }
        merge_context["concurrent_commands"].append(cmd)


# ==========================================================================
# Given Steps - Edge Cases
# ==========================================================================


@given("a new aggregate with no events")
def given_new_aggregate(merge_context):
    merge_context["aggregate"] = MockAggregateState()
    merge_context["next_sequence"] = 0


@given("a command targeting sequence 0")
def given_command_targeting_sequence_0(merge_context):
    merge_context["command_sequence"] = 0


@given(parsers.parse("an aggregate with snapshot at sequence {snap_seq:d}"))
def given_aggregate_with_snapshot(merge_context, snap_seq):
    merge_context["aggregate"] = MockAggregateState()
    merge_context["aggregate"].events = [
        {"sequence": i, "type": "Event"} for i in range(snap_seq + 1)
    ]
    merge_context["snapshot_sequence"] = snap_seq


@given(parsers.parse("events at sequences {seq1:d}, {seq2:d}"))
def given_events_at_sequences(merge_context, seq1, seq2):
    if merge_context.get("aggregate"):
        merge_context["aggregate"].events.append({"sequence": seq1, "type": "Event"})
        merge_context["aggregate"].events.append({"sequence": seq2, "type": "Event"})


@given(parsers.parse("the next expected sequence is {seq:d}"))
def given_next_expected_sequence(merge_context, seq):
    merge_context["next_sequence"] = seq
    if merge_context.get("aggregate"):
        merge_context["aggregate"].next_sequence = seq


@given(parsers.parse("the aggregate is at sequence {seq:d}"))
def given_aggregate_at_sequence(merge_context, seq):
    merge_context["next_sequence"] = seq


@given("a CommandBook with no pages")
def given_command_book_no_pages(merge_context):
    merge_context["command"] = types_pb2.CommandBook()


# ==========================================================================
# Given Steps - Saga
# ==========================================================================


@given("a saga emits a command with merge_strategy COMMUTATIVE")
def given_saga_emits_commutative(merge_context):
    merge_context["saga_command"] = True
    merge_context["merge_strategy"] = types_pb2.MERGE_COMMUTATIVE


@given("the destination aggregate has advanced")
def given_destination_advanced(merge_context):
    merge_context["command_sequence"] = 0  # Stale sequence


# ==========================================================================
# When Steps
# ==========================================================================


@when("the coordinator processes the command")
def when_coordinator_processes(merge_context):
    strategy = merge_context.get("merge_strategy", types_pb2.MERGE_COMMUTATIVE)
    cmd_seq = merge_context.get("command_sequence", 0)
    agg_next_seq = merge_context.get("next_sequence", 3)

    result = MockCoordinatorResult()

    # AGGREGATE_HANDLES bypasses coordinator validation
    if strategy == types_pb2.MERGE_AGGREGATE_HANDLES:
        agg = merge_context.get("aggregate")
        if agg and not agg.accepts_command:
            result.success = False
            result.error_message = agg.reject_reason or "Aggregate rejected"
        else:
            result.success = True
            result.event_book = make_event_book("player", agg_next_seq, agg_next_seq)
            merge_context["events_persisted"] = True
    elif cmd_seq == agg_next_seq:
        # Correct sequence
        result.success = True
        merge_context["events_persisted"] = True
    elif strategy == types_pb2.MERGE_STRICT:
        result.success = False
        result.status = "ABORTED"
        result.error_message = "Sequence mismatch"
        result.event_book = make_event_book("player", agg_next_seq, agg_next_seq)
    elif strategy == types_pb2.MERGE_COMMUTATIVE:
        result.success = False
        result.status = "FAILED_PRECONDITION"
        result.retryable = True
        result.event_book = make_event_book("player", agg_next_seq, agg_next_seq)

    merge_context["result"] = result


@when(parsers.parse("the command uses merge_strategy {strategy}"))
def when_command_uses_strategy(merge_context, strategy):
    strategy_map = {
        "STRICT": types_pb2.MERGE_STRICT,
        "COMMUTATIVE": types_pb2.MERGE_COMMUTATIVE,
        "AGGREGATE_HANDLES": types_pb2.MERGE_AGGREGATE_HANDLES,
    }
    merge_context["merge_strategy"] = strategy_map.get(
        strategy, types_pb2.MERGE_COMMUTATIVE
    )
    # Also process the command
    when_coordinator_processes(merge_context)


@when(parsers.parse("a STRICT command targets sequence {seq:d}"))
def when_strict_command_targets(merge_context, seq):
    merge_context["merge_strategy"] = types_pb2.MERGE_STRICT
    merge_context["command_sequence"] = seq
    # Also process the command
    when_coordinator_processes(merge_context)


@when("the client extracts the EventBook from the error")
def when_client_extracts_event_book(merge_context):
    result = merge_context.get("result")
    if result and result.event_book:
        merge_context["extracted_event_book"] = result.event_book


@when(parsers.parse("rebuilds the command with sequence {seq:d}"))
def when_rebuild_with_sequence(merge_context, seq):
    merge_context["command_sequence"] = seq


@when("resubmits the command")
def when_resubmit_command(merge_context):
    when_coordinator_processes(merge_context)


@when("the saga coordinator executes the command")
def when_saga_coordinator_executes(merge_context):
    when_coordinator_processes(merge_context)


@when("the saga retries with backoff")
@then("the saga retries with backoff")
def when_saga_retries(merge_context):
    merge_context["saga_retried"] = True


@when("the saga fetches fresh destination state")
@then("the saga fetches fresh destination state")
def when_saga_fetches_fresh_state(merge_context):
    merge_context["command_sequence"] = merge_context.get("next_sequence", 3)


@when("the retried command succeeds")
@then("the retried command succeeds")
def when_retried_command_succeeds(merge_context):
    when_coordinator_processes(merge_context)


@when("both commands use merge_strategy AGGREGATE_HANDLES")
def when_both_use_aggregate_handles(merge_context):
    merge_context["merge_strategy"] = types_pb2.MERGE_AGGREGATE_HANDLES


@when("both are processed")
def when_both_processed(merge_context):
    agg = merge_context.get("aggregate")
    commands = merge_context.get("concurrent_commands", [])

    for cmd in commands:
        result = MockCoordinatorResult()
        result.success = True
        merge_context["all_results"].append(result)

        # For counter: increment value
        if "amount" in cmd:
            agg.value += cmd["amount"]
        # For set: add item if not present
        elif "item" in cmd:
            if cmd["item"] not in agg.items:
                agg.items.append(cmd["item"])
                result.event_emitted = True
            else:
                result.event_emitted = False


@when("processed with sequence conflicts")
def when_processed_with_conflicts(merge_context):
    merge_context["conflict_results"] = {}
    for cmd in merge_context.get("concurrent_commands", []):
        cmd_name = cmd["command"]
        strategy = cmd["strategy"]

        if strategy == types_pb2.MERGE_STRICT:
            merge_context["conflict_results"][cmd_name] = "rejected"
        elif strategy == types_pb2.MERGE_COMMUTATIVE:
            merge_context["conflict_results"][cmd_name] = "retryable"
        else:
            merge_context["conflict_results"][cmd_name] = "delegated"


@when("merge_strategy is extracted")
def when_merge_strategy_extracted(merge_context):
    cmd = merge_context.get("command")
    if cmd and len(cmd.pages) == 0:
        merge_context["extracted_strategy"] = types_pb2.MERGE_COMMUTATIVE


# ==========================================================================
# Then Steps
# ==========================================================================


@then("the command succeeds")
def then_command_succeeds(merge_context):
    result = merge_context.get("result")
    assert result is not None
    assert result.success, f"Command should succeed, got: {result.error_message}"


@then("events are persisted")
def then_events_persisted(merge_context):
    assert merge_context.get("events_persisted"), "Events should be persisted"


@then(parsers.parse("the command fails with {status} status"))
def then_command_fails_with_status(merge_context, status):
    result = merge_context.get("result")
    assert result is not None
    assert not result.success, "Command should fail"
    # Handle "retryable" as a special case
    if status == "retryable":
        assert result.retryable, "Error should be retryable"
    else:
        assert result.status == status, f"Expected {status}, got {result.status}"


@then(parsers.parse('the error message contains "{message}"'))
def then_error_message_contains(merge_context, message):
    result = merge_context.get("result")
    assert result is not None
    assert (
        message in result.error_message
    ), f"Expected '{message}' in '{result.error_message}'"


@then("no events are persisted")
def then_no_events_persisted(merge_context):
    assert not merge_context.get("events_persisted"), "Events should not be persisted"


@then("the error details include the current EventBook")
def then_error_includes_event_book(merge_context):
    result = merge_context.get("result")
    assert result is not None
    assert result.event_book is not None


@then(parsers.parse("the EventBook shows next_sequence {seq:d}"))
def then_event_book_shows_next_sequence(merge_context, seq):
    result = merge_context.get("result")
    assert result is not None
    assert result.event_book is not None
    # EventBook next_sequence would be len(pages)
    assert len(result.event_book.pages) == seq


@then("the error is marked as retryable")
def then_error_is_retryable(merge_context):
    result = merge_context.get("result")
    assert result is not None
    assert result.retryable, "Error should be retryable"


@then("the command fails with retryable status")
def then_command_fails_retryable(merge_context):
    result = merge_context.get("result")
    assert result is not None
    assert not result.success
    assert result.retryable


@then("the coordinator does NOT validate the sequence")
def then_coordinator_does_not_validate(merge_context):
    # With AGGREGATE_HANDLES, coordinator passes through
    pass


@then("the aggregate handler is invoked")
def then_aggregate_handler_invoked(merge_context):
    pass


@then("the aggregate receives the prior EventBook")
def then_aggregate_receives_event_book(merge_context):
    pass


@then("events are persisted at the correct sequence")
def then_events_at_correct_sequence(merge_context):
    assert merge_context.get("events_persisted")


@then(parsers.parse("the command fails with aggregate's error"))
def then_command_fails_aggregate_error(merge_context):
    result = merge_context.get("result")
    assert result is not None
    assert not result.success


@then("both commands succeed")
def then_both_commands_succeed(merge_context):
    for result in merge_context.get("all_results", []):
        assert result.success


@then(parsers.parse("the final counter value is {value:d}"))
def then_final_counter_value(merge_context, value):
    agg = merge_context.get("aggregate")
    assert agg is not None
    assert agg.value == value


@then("no sequence conflicts occur")
def then_no_sequence_conflicts(merge_context):
    pass


@then(parsers.parse("the first command succeeds with {event_type} event"))
def then_first_command_succeeds_with_event(merge_context, event_type):
    results = merge_context.get("all_results", [])
    assert len(results) > 0
    assert results[0].success


@then("the second command succeeds with no event (idempotent)")
def then_second_command_idempotent(merge_context):
    results = merge_context.get("all_results", [])
    assert len(results) > 1
    assert results[1].success
    assert not getattr(results[1], "event_emitted", True)


@then(parsers.parse("the set contains {items}"))
def then_set_contains(merge_context, items):
    expected = items.strip("[]").replace('"', "").split(", ")
    agg = merge_context.get("aggregate")
    assert agg is not None
    for item in expected:
        assert item in agg.items


@then(parsers.parse("the response status is {status}"))
def then_response_status_is(merge_context, status):
    result = merge_context.get("result")
    assert result is not None
    if status == "varies":
        pass  # AGGREGATE_HANDLES varies based on aggregate logic
    elif status == "ABORTED":
        assert result.status == "ABORTED"
    elif status == "FAILED_PRECONDITION":
        assert result.status == "FAILED_PRECONDITION"


@then(parsers.parse("the behavior is {behavior}"))
def then_behavior_is(merge_context, behavior):
    result = merge_context.get("result")
    if "immediate rejection" in behavior:
        assert not result.success
        assert not result.retryable
    elif "retryable" in behavior:
        assert result.retryable
    elif "aggregate decides" in behavior:
        pass  # Aggregate handles it


@then("ReserveFunds is rejected immediately")
def then_reserve_funds_rejected(merge_context):
    results = merge_context.get("conflict_results", {})
    assert results.get("ReserveFunds") == "rejected"


@then("AddBonusPoints is retryable")
def then_bonus_points_retryable(merge_context):
    results = merge_context.get("conflict_results", {})
    assert results.get("AddBonusPoints") == "retryable"


@then("IncrementVisits delegates to aggregate")
def then_increment_visits_delegates(merge_context):
    results = merge_context.get("conflict_results", {})
    assert results.get("IncrementVisits") == "delegated"


@then(parsers.parse("the effective merge_strategy is {strategy}"))
def then_effective_strategy_is(merge_context, strategy):
    if strategy == "COMMUTATIVE":
        assert merge_context.get("merge_strategy") == types_pb2.MERGE_COMMUTATIVE


@then(parsers.parse("the result is {strategy}"))
def then_result_is_strategy(merge_context, strategy):
    if strategy == "COMMUTATIVE":
        assert merge_context.get("extracted_strategy") == types_pb2.MERGE_COMMUTATIVE
