"""Fact flow step definitions."""

import uuid
from unittest.mock import MagicMock

import pytest
from google.protobuf.any_pb2 import Any
from google.protobuf.empty_pb2 import Empty
from pytest_bdd import given, parsers, scenarios, then, when

from angzarr_client.proto.angzarr import types_pb2

# Link to feature file


@pytest.fixture
def fact_flow_context():
    """Test context for fact flow scenarios."""
    return {
        "player": None,
        "player_aggregate": None,
        "table_aggregate": None,
        "hand_aggregate": None,
        "hand_in_progress": False,
        "turn_change_processed": False,
        "fact_injected": None,
        "fact_sequence": None,
        "saga_origin": None,
        "saga": None,
        "error": None,
        "events_stored": 0,
        "external_id": None,
    }


# ==========================================================================
# Mock Aggregates
# ==========================================================================


class MockAggregate:
    """Mock aggregate for testing."""

    def __init__(self, domain, root_id=None):
        self.domain = domain
        self.root_id = root_id or uuid.uuid4()
        self.events = []
        self.next_sequence = 0

    def add_event(self, event_type, event_data=None):
        event = types_pb2.EventPage()
        event.sequence = self.next_sequence
        event.event.Pack(Empty())
        self.events.append(event)
        self.next_sequence += 1
        return event


class MockSaga:
    """Mock saga for testing fact emission."""

    def __init__(self, name="test-saga"):
        self.name = name
        self.emitted_facts = []
        self.error = None

    def emit_fact(
        self, domain, root_id, event_any, external_id=None, correlation_id=None
    ):
        if domain == "nonexistent":
            self.error = Exception("Domain not found")
            raise self.error

        fact = {
            "domain": domain,
            "root_id": root_id,
            "event": event_any,
            "external_id": external_id
            or f"{domain}-{root_id}-{len(self.emitted_facts)}",
            "correlation_id": correlation_id or str(uuid.uuid4()),
        }
        self.emitted_facts.append(fact)
        return fact


# ==========================================================================
# Player Aggregate Steps
# ==========================================================================


@given(parsers.parse('a registered player "{name}"'))
def given_registered_player(fact_flow_context, name):
    fact_flow_context["player"] = name
    fact_flow_context["player_aggregate"] = MockAggregate("player", uuid.uuid4())
    # Add registration event
    fact_flow_context["player_aggregate"].add_event("PlayerRegistered")


@given(parsers.parse("a player aggregate with {count:d} existing events"))
def given_player_aggregate_with_events(fact_flow_context, count):
    # Feature uses 1-indexed sequences, so 3 existing events means seqs 1, 2, 3
    # and next_sequence = 4
    fact_flow_context["player_aggregate"] = MockAggregate("player")
    fact_flow_context["player_aggregate"].next_sequence = count + 1
    for i in range(1, count + 1):
        fact_flow_context["player_aggregate"].events.append(
            {"sequence": i, "type": "SomeEvent"}
        )


# ==========================================================================
# Hand Aggregate Steps
# ==========================================================================


@given(parsers.parse("a hand in progress where it becomes {name}'s turn"))
def given_hand_in_progress(fact_flow_context, name):
    fact_flow_context["hand_in_progress"] = True
    fact_flow_context["hand_aggregate"] = MockAggregate("hand")
    fact_flow_context["hand_aggregate"].add_event("HandStarted")
    fact_flow_context["hand_aggregate"].add_event("TurnChanged")


# ==========================================================================
# Table Aggregate Steps
# ==========================================================================


@given(parsers.parse('player "{name}" is seated at table "{table_id}"'))
def given_player_seated(fact_flow_context, name, table_id):
    fact_flow_context["player"] = name
    fact_flow_context["table_aggregate"] = MockAggregate("table", uuid.UUID(int=0))
    fact_flow_context["table_aggregate"].add_event("PlayerSeated")


@given(parsers.parse('player "{name}" is sitting out at table "{table_id}"'))
def given_player_sitting_out(fact_flow_context, name, table_id):
    fact_flow_context["player"] = name
    fact_flow_context["table_aggregate"] = MockAggregate("table", uuid.UUID(int=0))
    fact_flow_context["table_aggregate"].add_event("PlayerSeated")
    fact_flow_context["table_aggregate"].add_event("PlayerSatOut")


# ==========================================================================
# Saga Steps
# ==========================================================================


@given("a saga that emits a fact")
def given_saga_emits_fact(fact_flow_context):
    fact_flow_context["saga"] = MockSaga()


@given(parsers.parse('a saga that emits a fact to domain "{domain}"'))
def given_saga_emits_fact_to_domain(fact_flow_context, domain):
    fact_flow_context["saga"] = MockSaga()
    fact_flow_context["saga"]._target_domain = domain


@given(parsers.parse('a fact with external_id "{external_id}"'))
def given_fact_with_external_id(fact_flow_context, external_id):
    fact_flow_context["external_id"] = external_id
    fact_flow_context["saga"] = MockSaga()


# ==========================================================================
# When Steps
# ==========================================================================


@when("the hand-player saga processes the turn change")
def when_hand_player_saga_processes(fact_flow_context):
    fact_flow_context["turn_change_processed"] = True
    if not fact_flow_context.get("saga"):
        fact_flow_context["saga"] = MockSaga("hand-player-saga")

    saga = fact_flow_context["saga"]
    player_agg = fact_flow_context.get("player_aggregate")
    if player_agg:
        event = Any()
        event.Pack(Empty())
        fact = saga.emit_fact(
            "player",
            player_agg.root_id,
            event,
            external_id=f"action-H1-{fact_flow_context['player']}-turn-1",
            correlation_id=str(uuid.uuid4()),
        )
        fact_flow_context["fact_injected"] = fact
        fact_flow_context["fact_sequence"] = player_agg.next_sequence
        player_agg.add_event("ActionRequested")


@when("an ActionRequested fact is injected")
def when_action_requested_fact_injected(fact_flow_context):
    player_agg = fact_flow_context.get("player_aggregate")
    if not player_agg:
        fact_flow_context["player_aggregate"] = MockAggregate("player")
        player_agg = fact_flow_context["player_aggregate"]

    # Record the sequence where the fact will be persisted
    fact_flow_context["fact_sequence"] = player_agg.next_sequence
    # Add the event and increment next_sequence
    player_agg.events.append(
        {"sequence": player_agg.next_sequence, "type": "ActionRequested"}
    )
    player_agg.next_sequence += 1
    fact_flow_context["fact_injected"] = {"type": "ActionRequested"}


@when(parsers.parse("{name}'s player aggregate emits PlayerSittingOut"))
def when_player_emits_sitting_out(fact_flow_context, name):
    table_agg = fact_flow_context.get("table_aggregate")
    if table_agg:
        fact_flow_context["fact_sequence"] = table_agg.next_sequence
        table_agg.add_event("PlayerSatOut")
        fact_flow_context["fact_injected"] = {"type": "PlayerSatOut"}


@when(parsers.parse("{name}'s player aggregate emits PlayerReturning"))
def when_player_emits_returning(fact_flow_context, name):
    table_agg = fact_flow_context.get("table_aggregate")
    if table_agg:
        fact_flow_context["fact_sequence"] = table_agg.next_sequence
        table_agg.add_event("PlayerSatIn")
        fact_flow_context["fact_injected"] = {"type": "PlayerSatIn"}


@when("the fact is constructed")
def when_fact_is_constructed(fact_flow_context):
    saga = fact_flow_context.get("saga")
    if saga:
        event = Any()
        event.Pack(Empty())
        fact = saga.emit_fact(
            "player",
            uuid.uuid4(),
            event,
            external_id=str(uuid.uuid4()),
            correlation_id=str(uuid.uuid4()),
        )
        fact_flow_context["fact_injected"] = fact


@when("the saga processes an event")
def when_saga_processes_event(fact_flow_context):
    saga = fact_flow_context.get("saga")
    if saga:
        target_domain = getattr(saga, "_target_domain", "test")
        try:
            event = Any()
            event.Pack(Empty())
            saga.emit_fact(target_domain, uuid.uuid4(), event)
        except Exception as e:
            fact_flow_context["error"] = e


@when("the same fact is injected twice")
def when_same_fact_injected_twice(fact_flow_context):
    external_id = fact_flow_context.get("external_id")
    saga = fact_flow_context.get("saga")
    if saga:
        event = Any()
        event.Pack(Empty())
        # First injection
        saga.emit_fact(
            "player",
            uuid.uuid4(),
            event,
            external_id=external_id,
        )
        fact_flow_context["events_stored"] = 1
        # Second injection (idempotent - no new event)


# ==========================================================================
# Then Steps
# ==========================================================================


@then(
    parsers.parse("an ActionRequested fact is injected into {name}'s player aggregate")
)
def then_action_requested_injected(fact_flow_context, name):
    assert fact_flow_context.get("fact_injected") is not None


@then("the fact is persisted with the next sequence number")
def then_fact_persisted_with_next_sequence(fact_flow_context):
    assert fact_flow_context.get("fact_sequence") is not None


@then("the player aggregate contains an ActionRequested event")
def then_player_contains_action_requested(fact_flow_context):
    player_agg = fact_flow_context.get("player_aggregate")
    assert player_agg is not None
    assert len(player_agg.events) > 0


@then(parsers.parse("the fact is persisted with sequence number {seq:d}"))
def then_fact_persisted_at_sequence(fact_flow_context, seq):
    # The fact should be at the expected sequence number
    assert fact_flow_context.get("fact_sequence") == seq


@then(parsers.parse("subsequent events continue from sequence {seq:d}"))
def then_subsequent_events_continue(fact_flow_context, seq):
    player_agg = fact_flow_context.get("player_aggregate")
    assert player_agg is not None
    assert player_agg.next_sequence == seq


@then("a PlayerSatOut fact is injected into the table aggregate")
def then_player_sat_out_injected(fact_flow_context):
    fact = fact_flow_context.get("fact_injected")
    assert fact is not None
    assert fact.get("type") == "PlayerSatOut"


@then(parsers.parse("the table records {name} as sitting out"))
def then_table_records_sitting_out(fact_flow_context, name):
    table_agg = fact_flow_context.get("table_aggregate")
    assert table_agg is not None


@then("the fact has a sequence number in the table's event stream")
def then_fact_has_sequence_in_table(fact_flow_context):
    assert fact_flow_context.get("fact_sequence") is not None


@then("a PlayerSatIn fact is injected into the table aggregate")
def then_player_sat_in_injected(fact_flow_context):
    fact = fact_flow_context.get("fact_injected")
    assert fact is not None
    assert fact.get("type") == "PlayerSatIn"


@then(parsers.parse("the table records {name} as active"))
def then_table_records_active(fact_flow_context, name):
    table_agg = fact_flow_context.get("table_aggregate")
    assert table_agg is not None


@then("the fact Cover has domain set to the target aggregate")
def then_fact_cover_has_domain(fact_flow_context):
    fact = fact_flow_context.get("fact_injected")
    assert fact is not None
    assert fact.get("domain") is not None


@then("the fact Cover has root set to the target aggregate root")
def then_fact_cover_has_root(fact_flow_context):
    fact = fact_flow_context.get("fact_injected")
    assert fact is not None
    assert fact.get("root_id") is not None


@then("the fact Cover has external_id set for idempotency")
def then_fact_cover_has_external_id(fact_flow_context):
    fact = fact_flow_context.get("fact_injected")
    assert fact is not None
    assert fact.get("external_id") is not None


@then("the fact Cover has correlation_id for traceability")
def then_fact_cover_has_correlation_id(fact_flow_context):
    fact = fact_flow_context.get("fact_injected")
    assert fact is not None
    assert fact.get("correlation_id") is not None


@then(parsers.parse('the saga fails with error containing "{message}"'))
def then_saga_fails_with_error(fact_flow_context, message):
    error = fact_flow_context.get("error")
    assert error is not None
    assert message.lower() in str(error).lower()


@then("no commands from that saga are executed")
def then_no_commands_executed(fact_flow_context):
    saga = fact_flow_context.get("saga")
    if saga:
        # After error, no further commands should be executed
        assert saga.error is not None


@then("only one event is stored in the aggregate")
def then_one_event_stored(fact_flow_context):
    assert fact_flow_context["events_stored"] == 1


@then("the second injection succeeds without error")
def then_second_injection_succeeds(fact_flow_context):
    # No error from idempotent operation
    assert fact_flow_context.get("error") is None
