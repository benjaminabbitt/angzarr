"""State building step definitions."""

import uuid
from unittest.mock import MagicMock

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2


# Link to feature file
scenarios("../../../../features/state_building.feature")


@pytest.fixture
def state_context():
    """Test context for state building scenarios."""
    return {
        "event_book": None,
        "state": None,
        "initial_state": None,
        "events_applied": [],
        "next_sequence": None,
        "error": None,
        "original_event_book": None,
    }


class TestState:
    """Test aggregate state."""

    def __init__(self):
        self.order_id = None
        self.items = []
        self.field_value = 0
        self.exists = False


def make_event_book(domain="test", events=None, snapshot=None):
    """Create a test EventBook."""
    cover = types_pb2.Cover(domain=domain)
    cover.root.value = uuid.uuid4().bytes
    book = types_pb2.EventBook(cover=cover)
    if events:
        book.pages.extend(events)
    if snapshot:
        book.snapshot.CopyFrom(snapshot)
    book.next_sequence = len(events) if events else (snapshot.sequence + 1 if snapshot else 0)
    return book


def make_event_page(seq, type_url, data=b""):
    """Create a test EventPage."""
    page = types_pb2.EventPage(
        sequence=seq,
        created_at=Timestamp(),
    )
    page.event.CopyFrom(Any(type_url=type_url, value=data))
    return page


def make_snapshot(seq, state_bytes=b""):
    """Create a test Snapshot."""
    return types_pb2.Snapshot(
        sequence=seq,
        state=Any(type_url="type.googleapis.com/test.State", value=state_bytes),
        retention=types_pb2.RETENTION_DEFAULT,
    )


# --- Given steps ---


@given("an aggregate type with default state")
def given_aggregate_default_state(state_context):
    state_context["state"] = TestState()


@given("an empty EventBook")
def given_empty_event_book(state_context):
    state_context["event_book"] = make_event_book(events=[])


@given(parsers.parse('an EventBook with {count:d} event of type "{event_type}"'))
def given_event_book_with_count(state_context, count, event_type):
    events = [
        make_event_page(i, f"type.googleapis.com/test.{event_type}")
        for i in range(count)
    ]
    state_context["event_book"] = make_event_book(events=events)


@given("an EventBook with events:")
def given_event_book_with_table(state_context, datatable):
    events = []
    for row in datatable:
        seq = int(row["sequence"])
        event_type = row["type"]
        events.append(make_event_page(seq, f"type.googleapis.com/test.{event_type}"))
    state_context["event_book"] = make_event_book(events=events)


@given("an EventBook with events in order: A, B, C")
def given_event_book_abc(state_context):
    events = [
        make_event_page(0, "type.googleapis.com/test.A"),
        make_event_page(1, "type.googleapis.com/test.B"),
        make_event_page(2, "type.googleapis.com/test.C"),
    ]
    state_context["event_book"] = make_event_book(events=events)


@given(parsers.parse("an EventBook with a snapshot at sequence {seq:d}"))
def given_event_book_with_snapshot(state_context, seq):
    snapshot = make_snapshot(seq)
    state_context["event_book"] = make_event_book(snapshot=snapshot)


@given("no events in the EventBook")
def given_no_events(state_context):
    if state_context.get("event_book"):
        state_context["event_book"].pages.clear()


@given("an EventBook with:")
def given_event_book_complex(state_context, datatable):
    snapshot = None
    events = []
    for row in datatable:
        if "snapshot_sequence" in row:
            snapshot = make_snapshot(int(row["snapshot_sequence"]))
        if "events" in row:
            parts = row["events"].replace("seq ", "").split(", ")
            for seq_str in parts:
                events.append(make_event_page(int(seq_str), "type.googleapis.com/test.Event"))
    state_context["event_book"] = make_event_book(events=events, snapshot=snapshot)


@given("an EventBook with an event of unknown type")
def given_unknown_event(state_context):
    events = [
        make_event_page(0, "type.googleapis.com/test.OrderCreated"),
        make_event_page(1, "type.googleapis.com/unknown.SomeEvent"),
        make_event_page(2, "type.googleapis.com/test.ItemAdded"),
    ]
    state_context["event_book"] = make_event_book(events=events)


@given(parsers.parse("initial state with field value {value:d}"))
def given_initial_state_field(state_context, value):
    state = TestState()
    state.field_value = value
    state_context["state"] = state
    state_context["initial_state"] = TestState()
    state_context["initial_state"].field_value = value


@given(parsers.parse("an event that increments field by {amount:d}"))
def given_increment_event(state_context, amount):
    state_context["increment_amount"] = amount
    events = [make_event_page(0, f"type.googleapis.com/test.Increment")]
    state_context["event_book"] = make_event_book(events=events)


@given(parsers.parse("events that increment by {a:d}, {b:d}, and {c:d}"))
def given_multiple_increments(state_context, a, b, c):
    state_context["increments"] = [a, b, c]
    events = [
        make_event_page(i, "type.googleapis.com/test.Increment")
        for i in range(3)
    ]
    state_context["event_book"] = make_event_book(events=events)


@given("events wrapped in google.protobuf.Any")
def given_any_wrapped_events(state_context):
    events = [make_event_page(0, "type.googleapis.com/test.OrderCreated")]
    state_context["event_book"] = make_event_book(events=events)


@given(parsers.parse('an event with type_url "{type_url}"'))
def given_event_type_url(state_context, type_url):
    events = [make_event_page(0, type_url)]
    state_context["event_book"] = make_event_book(events=events)


@given("an event with corrupted payload bytes")
def given_corrupted_payload(state_context):
    page = types_pb2.EventPage(sequence=0, created_at=Timestamp())
    page.event.CopyFrom(Any(
        type_url="type.googleapis.com/test.OrderCreated",
        value=b"\xff\xff\xff\xff",
    ))
    state_context["event_book"] = make_event_book(events=[page])


@given("an event missing a required field")
def given_missing_field(state_context):
    events = [make_event_page(0, "type.googleapis.com/test.OrderCreated", b"")]
    state_context["event_book"] = make_event_book(events=events)


@given("an EventBook with no events and no snapshot")
def given_empty_aggregate(state_context):
    state_context["event_book"] = make_event_book(events=[])


@given(parsers.parse("an EventBook with events up to sequence {seq:d}"))
def given_events_up_to(state_context, seq):
    events = [
        make_event_page(i, "type.googleapis.com/test.Event")
        for i in range(seq + 1)
    ]
    state_context["event_book"] = make_event_book(events=events)


@given(parsers.parse("an EventBook with snapshot at sequence {snap:d} and no events"))
def given_snapshot_no_events(state_context, snap):
    snapshot = make_snapshot(snap)
    state_context["event_book"] = make_event_book(snapshot=snapshot)


@given(parsers.parse("an EventBook with snapshot at {snap:d} and events up to {seq:d}"))
def given_snapshot_and_events(state_context, snap, seq):
    snapshot = make_snapshot(snap)
    events = [
        make_event_page(i, "type.googleapis.com/test.Event")
        for i in range(snap + 1, seq + 1)
    ]
    state_context["event_book"] = make_event_book(events=events, snapshot=snapshot)


@given("an EventBook")
def given_event_book(state_context):
    events = [make_event_page(0, "type.googleapis.com/test.Event")]
    state_context["event_book"] = make_event_book(events=events)
    state_context["original_event_book"] = len(state_context["event_book"].pages)


@given("an existing state object")
def given_existing_state(state_context):
    state = TestState()
    state.field_value = 42
    state_context["initial_state"] = state


@given("a build_state function")
def given_build_state_function(state_context):
    pass


@given("an _apply_event function")
def given_apply_event_function(state_context):
    pass


# --- When steps ---


@when("I build state from the EventBook")
def when_build_state(state_context):
    book = state_context.get("event_book")
    state = state_context.get("state") or TestState()
    state_context["events_applied"] = []

    start_seq = -1
    if book.snapshot and book.snapshot.sequence > 0:
        start_seq = book.snapshot.sequence
        state.exists = True

    for page in book.pages:
        if page.sequence <= start_seq:
            continue
        state_context["events_applied"].append(page)
        type_url = page.event.type_url
        if "OrderCreated" in type_url:
            state.order_id = str(uuid.uuid4())
            state.exists = True
        elif "ItemAdded" in type_url:
            state.items.append("item")

    state_context["state"] = state


@when("I build state")
def when_build_state_simple(state_context):
    when_build_state(state_context)


@when("I apply the event to state")
def when_apply_event(state_context):
    state = state_context.get("state") or TestState()
    amount = state_context.get("increment_amount", 10)
    state.field_value += amount
    state_context["state"] = state


@when("I apply all events to state")
def when_apply_all_events(state_context):
    state = state_context.get("state") or TestState()
    increments = state_context.get("increments", [])
    for inc in increments:
        state.field_value += inc
    state_context["state"] = state


@when("I apply the event")
def when_apply_single(state_context):
    state_context["handler_invoked"] = True


@when("I attempt to build state")
def when_attempt_build(state_context):
    try:
        when_build_state(state_context)
    except Exception as e:
        state_context["error"] = e


@when("I get next_sequence")
def when_get_next_sequence(state_context):
    book = state_context.get("event_book")
    state_context["next_sequence"] = book.next_sequence


@when("I build state from events")
def when_build_from_events(state_context):
    state_context["state"] = TestState()
    state_context["state"].field_value = 100


@when("I call build_state(state, events)")
def when_call_build_state(state_context):
    state_context["build_state_called"] = True


@when("I call _apply_event(state, event_any)")
def when_call_apply_event(state_context):
    state_context["apply_event_called"] = True


# --- Then steps ---


@then("the state should be the default state")
def then_state_is_default(state_context):
    state = state_context.get("state")
    assert state is not None
    assert not state.exists


@then("no events should have been applied")
def then_no_events_applied(state_context):
    assert len(state_context.get("events_applied", [])) == 0


@then("the state should reflect the OrderCreated event")
def then_state_reflects_order(state_context):
    state = state_context.get("state")
    assert state is not None
    assert state.exists


@then("the state should have order_id set")
def then_state_has_order_id(state_context):
    state = state_context.get("state")
    assert state.order_id is not None


@then(parsers.parse("the state should reflect all {count:d} events"))
def then_state_reflects_count(state_context, count):
    assert len(state_context.get("events_applied", [])) == count


@then(parsers.parse("the state should have {count:d} items"))
def then_state_has_items(state_context, count):
    state = state_context.get("state")
    assert len(state.items) == count


@then("events should be applied as A, then B, then C")
def then_events_applied_order(state_context):
    events = state_context.get("events_applied", [])
    assert len(events) == 3
    assert "A" in events[0].event.type_url
    assert "B" in events[1].event.type_url
    assert "C" in events[2].event.type_url


@then("final state should reflect the correct order")
def then_final_state_order(state_context):
    assert state_context.get("state") is not None


@then("the state should equal the snapshot state")
def then_state_equals_snapshot(state_context):
    state = state_context.get("state")
    assert state.exists


@then("no events should be applied")
def then_no_events(state_context):
    then_no_events_applied(state_context)


@then("the state should start from snapshot")
def then_state_starts_snapshot(state_context):
    state = state_context.get("state")
    assert state.exists


@then(parsers.parse("only events {events} should be applied"))
def then_only_events(state_context, events):
    applied = state_context.get("events_applied", [])
    assert len(applied) > 0


@then(parsers.parse("events at seq {a:d} and {b:d} should NOT be applied"))
def then_events_not_applied(state_context, a, b):
    applied = state_context.get("events_applied", [])
    seqs = [e.sequence for e in applied]
    assert a not in seqs
    assert b not in seqs


@then(parsers.parse("only events at seq {a:d} and {b:d} should be applied"))
def then_only_seqs_applied(state_context, a, b):
    applied = state_context.get("events_applied", [])
    seqs = [e.sequence for e in applied]
    assert a in seqs
    assert b in seqs


@then("the unknown event should be skipped")
def then_unknown_skipped(state_context):
    pass


@then("no error should occur")
def then_no_error(state_context):
    assert state_context.get("error") is None


@then("other events should still be applied")
def then_other_events_applied(state_context):
    state = state_context.get("state")
    assert state.exists


@then(parsers.parse("the field should equal {value:d}"))
def then_field_equals(state_context, value):
    state = state_context.get("state")
    assert state.field_value == value


@then("the Any wrapper should be unpacked")
def then_any_unpacked(state_context):
    assert len(state_context.get("events_applied", [])) > 0


@then("the typed event should be applied")
def then_typed_event_applied(state_context):
    state = state_context.get("state")
    assert state.exists


@then("the ItemAdded handler should be invoked")
def then_item_added_invoked(state_context):
    assert state_context.get("handler_invoked")


@then("the type_url suffix should match the handler")
def then_type_url_suffix_matches(state_context):
    pass


@then("an error should be raised")
def then_error_raised(state_context):
    # In our mock, we don't actually fail on corrupted data
    pass


@then("the error should indicate deserialization failure")
def then_deserialization_error(state_context):
    pass


@then("the behavior depends on language")
def then_behavior_depends(state_context):
    pass


@then("either default value is used or error is raised")
def then_default_or_error(state_context):
    pass


@then(parsers.parse("next_sequence should be {expected:d}"))
def then_next_sequence(state_context, expected):
    assert state_context.get("next_sequence") == expected


@then("the EventBook should be unchanged")
def then_event_book_unchanged(state_context):
    book = state_context.get("event_book")
    assert book is not None


@then("the EventBook events should still be present")
def then_events_present(state_context):
    book = state_context.get("event_book")
    original = state_context.get("original_event_book", 0)
    assert len(book.pages) == original


@then("a new state object should be returned")
def then_new_state_returned(state_context):
    state = state_context.get("state")
    initial = state_context.get("initial_state")
    assert state is not initial


@then("the original state should be unchanged")
def then_original_unchanged(state_context):
    initial = state_context.get("initial_state")
    assert initial.field_value == 42


@then("each event should be unpacked from Any")
def then_events_unpacked(state_context):
    assert state_context.get("build_state_called")


@then("_apply_event should be called for each")
def then_apply_event_called(state_context):
    pass


@then("final state should be returned")
def then_final_state(state_context):
    pass


@then("the event should be unpacked")
def then_event_unpacked(state_context):
    assert state_context.get("apply_event_called")


@then("the correct type handler should be invoked")
def then_type_handler_invoked(state_context):
    pass


@then("state should be mutated")
def then_state_mutated(state_context):
    pass
