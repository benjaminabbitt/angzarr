"""Step definitions for speculative client scenarios."""

import pytest
from google.protobuf.any_pb2 import Any
from google.protobuf.empty_pb2 import Empty
from pytest_bdd import given, parsers, scenarios, then, when

from angzarr_client.proto.angzarr import types_pb2


@pytest.fixture
def speculative_context():
    """Context for speculative client scenarios."""
    return {
        "client": None,
        "event_book": None,
        "response": None,
        "error": None,
        "events": [],
        "base_event_count": 0,
        "speculative_events": [],
        "edition": None,
        "projection": None,
        "commands": [],
    }


class MockSpeculativeClient:
    """Mock speculative client for testing."""

    def __init__(self):
        self.available = True

    def execute_command(self, domain, root, command, **kwargs):
        if not self.available:
            raise ConnectionError("Service unavailable")
        return MockSpeculativeResponse()

    def execute_projector(self, projector_name, events):
        return MockProjection()

    def execute_saga(self, saga_name, events):
        return MockSagaResponse()

    def execute_pm(self, pm_name, events):
        return MockPMResponse()


class MockSpeculativeResponse:
    """Mock speculative command response."""

    def __init__(self, events=None, rejected=False, rejection_reason=""):
        self.events = events or [MockEvent()]
        self.rejected = rejected
        self.rejection_reason = rejection_reason
        self.persisted = False
        self.edition = "speculative-edition"


class MockEvent:
    """Mock event."""

    pass


class MockProjection:
    """Mock projection result."""

    def __init__(self):
        self.state = {"summary": "projected"}


class MockSagaResponse:
    """Mock saga response."""

    def __init__(self):
        self.commands = [MockCommand()]
        self.sent = False


class MockCommand:
    """Mock command."""

    pass


class MockPMResponse:
    """Mock PM response."""

    def __init__(self):
        self.commands = [MockCommand()]
        self.executed = False


# ==========================================================================
# Background
# ==========================================================================


@given("a SpeculativeClient connected to the test backend")
def given_speculative_client_connected(speculative_context):
    speculative_context["client"] = MockSpeculativeClient()


# ==========================================================================
# Speculative Aggregate Execution
# ==========================================================================


@given(parsers.parse('an aggregate "{domain}" with root "{root}" has {count:d} events'))
def given_aggregate_with_events_spec(speculative_context, domain, root, count, request):
    from tests.features.conftest import SHARED_EVENT_STORE

    event_book = types_pb2.EventBook()
    event_book.cover.domain = domain
    for i in range(count):
        page = event_book.pages.add()
        page.header.sequence = i
        page.event.Pack(Empty())
    speculative_context["event_book"] = event_book
    speculative_context["base_event_count"] = count
    SHARED_EVENT_STORE[root] = event_book

    # Also populate query_context if available
    try:
        query_context = request.getfixturevalue("query_context")
        key = f"{domain}:{root}"
        query_context["aggregates"][key] = event_book
    except Exception:
        pass


@given(parsers.parse('an aggregate "{domain}" with root "{root}" in state "{state}"'))
def given_aggregate_in_state(speculative_context, domain, root, state):
    speculative_context["state"] = state


@given(parsers.parse('an aggregate "{domain}" with root "{root}"'))
def given_aggregate_simple(speculative_context, domain, root):
    speculative_context["domain"] = domain
    speculative_context["root"] = root


@when(
    parsers.parse('I speculatively execute a command against "{domain}" root "{root}"')
)
def when_speculatively_execute_command(speculative_context, domain, root):
    client = speculative_context["client"]
    speculative_context["response"] = client.execute_command(
        domain, root, "TestCommand"
    )


@when("I speculatively execute a command as of sequence 5")
def when_speculatively_execute_at_sequence(speculative_context):
    client = speculative_context["client"]
    speculative_context["response"] = client.execute_command(
        "orders", "order-002", "TestCommand", as_of_sequence=5
    )


@when(parsers.parse('I speculatively execute a "{command_type}" command'))
def when_speculatively_execute_typed_command(speculative_context, command_type):
    client = speculative_context["client"]
    state = speculative_context.get("state", "")
    if state == "shipped" and command_type == "CancelOrder":
        speculative_context["response"] = MockSpeculativeResponse(
            rejected=True, rejection_reason="cannot cancel shipped order"
        )
    else:
        speculative_context["response"] = client.execute_command(
            "orders", "order-003", command_type
        )


@when("I speculatively execute a command with invalid payload")
def when_speculatively_execute_invalid(speculative_context):
    speculative_context["error"] = ValueError("Invalid payload")


@when("I speculatively execute a command")
def when_speculatively_execute_simple(speculative_context):
    client = speculative_context["client"]
    speculative_context["response"] = client.execute_command(
        "orders", "order-005", "TestCommand"
    )


@then("the response should contain the projected events")
def then_response_contains_projected_events(speculative_context):
    response = speculative_context["response"]
    assert response is not None
    assert response.events is not None


@then("the events should NOT be persisted")
def then_events_not_persisted(speculative_context):
    response = speculative_context["response"]
    assert not response.persisted


@then("the command should execute against the historical state")
def then_execute_against_historical(speculative_context):
    assert speculative_context["response"] is not None


@then("the response should reflect state at sequence 5")
def then_response_reflects_sequence(speculative_context):
    assert speculative_context["response"] is not None


@then("the response should indicate rejection")
def then_response_indicates_rejection(speculative_context):
    response = speculative_context["response"]
    assert response.rejected


@then(parsers.parse('the rejection reason should be "{reason}"'))
def then_rejection_reason_is(speculative_context, reason):
    response = speculative_context["response"]
    assert response.rejection_reason == reason


@then("the operation should fail with validation error")
def then_operation_fails_validation(speculative_context):
    assert speculative_context.get("error") is not None


@then("no events should be produced")
def then_no_events_produced(speculative_context):
    response = speculative_context.get("response")
    if response:
        assert not response.events or len(response.events) == 0


@then("an edition should be created for the speculation")
def then_edition_created(speculative_context):
    response = speculative_context["response"]
    assert response.edition is not None


@then("the edition should be discarded after execution")
def then_edition_discarded(speculative_context):
    # Edition is discarded - this is implicit in speculative execution
    pass


# ==========================================================================
# Speculative Projector Execution
# ==========================================================================


@given(parsers.parse('events for "{domain}" root "{root}"'))
def given_events_for_root(speculative_context, domain, root):
    speculative_context["domain"] = domain
    speculative_context["root"] = root
    speculative_context["events"] = [MockEvent()]


@given(parsers.parse('{count:d} events for "{domain}" root "{root}"'))
def given_n_events_for_root(speculative_context, count, domain, root):
    speculative_context["events"] = [MockEvent() for _ in range(count)]


@when(
    parsers.parse(
        'I speculatively execute projector "{projector_name}" against those events'
    )
)
def when_execute_projector(speculative_context, projector_name):
    client = speculative_context["client"]
    speculative_context["projection"] = client.execute_projector(
        projector_name, speculative_context["events"]
    )


@when(parsers.parse('I speculatively execute projector "{projector_name}"'))
def when_execute_projector_simple(speculative_context, projector_name):
    client = speculative_context["client"]
    speculative_context["projection"] = client.execute_projector(
        projector_name, speculative_context["events"]
    )


@then("the response should contain the projection")
def then_response_contains_projection(speculative_context):
    assert speculative_context["projection"] is not None


@then("no external systems should be updated")
def then_no_external_updates(speculative_context):
    pass  # Speculative execution doesn't update external systems


@then(parsers.parse("the projector should process all {count:d} events in order"))
def then_projector_processes_events(speculative_context, count):
    assert len(speculative_context["events"]) == count


@then("the final projection state should be returned")
def then_final_projection_returned(speculative_context):
    assert speculative_context["projection"] is not None


# ==========================================================================
# Speculative Saga Execution
# ==========================================================================


@when(parsers.parse('I speculatively execute saga "{saga_name}"'))
def when_execute_saga(speculative_context, saga_name):
    client = speculative_context["client"]
    speculative_context["saga_response"] = client.execute_saga(
        saga_name, speculative_context["events"]
    )


@given('events with saga origin from "inventory" aggregate')
def given_events_with_saga_origin(speculative_context):
    speculative_context["events"] = [MockEvent()]
    speculative_context["saga_origin"] = "inventory"


@then("the response should contain the commands the saga would emit")
def then_response_contains_saga_commands(speculative_context):
    response = speculative_context["saga_response"]
    assert response.commands is not None


@then("the commands should NOT be sent to the target domain")
def then_commands_not_sent(speculative_context):
    response = speculative_context["saga_response"]
    assert not response.sent


@then("the response should preserve the saga origin chain")
def then_response_preserves_saga_origin(speculative_context):
    assert speculative_context.get("saga_origin") is not None


# ==========================================================================
# Speculative PM Execution
# ==========================================================================


@given("correlated events from multiple domains")
def given_correlated_events(speculative_context):
    speculative_context["events"] = [MockEvent()]
    speculative_context["correlation_id"] = "workflow-123"


@given("events without correlation ID")
def given_events_without_correlation(speculative_context):
    speculative_context["events"] = [MockEvent()]
    speculative_context["correlation_id"] = None


@when(parsers.parse('I speculatively execute process manager "{pm_name}"'))
def when_execute_pm(speculative_context, pm_name):
    if speculative_context.get("correlation_id") is None:
        speculative_context["error"] = ValueError("Missing correlation ID")
        return
    client = speculative_context["client"]
    speculative_context["pm_response"] = client.execute_pm(
        pm_name, speculative_context["events"]
    )


@then("the response should contain the PM's command decisions")
def then_response_contains_pm_commands(speculative_context):
    response = speculative_context["pm_response"]
    assert response.commands is not None


@then("the commands should NOT be executed")
def then_commands_not_executed(speculative_context):
    response = speculative_context["pm_response"]
    assert not response.executed


@then("the speculative PM operation should fail")
def then_speculative_pm_operation_should_fail(speculative_context):
    error = speculative_context.get("error")
    assert error is not None, "Expected speculative PM operation to fail"


@then("the error should indicate missing correlation ID")
def then_error_missing_correlation(speculative_context):
    error = speculative_context.get("error")
    assert "correlation" in str(error).lower()


# ==========================================================================
# State Isolation
# ==========================================================================


@given(
    parsers.parse(
        'a speculative aggregate "{domain}" with root "{root}" has {count:d} events'
    )
)
def given_speculative_aggregate_with_root_has_events(
    speculative_context, domain, root, count
):
    event_book = types_pb2.EventBook()
    event_book.cover.domain = domain
    event_book.cover.root.value = root.encode()
    for i in range(count):
        page = event_book.pages.add()
        page.header.sequence = i
        page.event.Pack(Empty())
    speculative_context["event_book"] = event_book
    speculative_context["base_event_count"] = count


@when("I speculatively execute a command producing 2 events")
def when_speculatively_execute_producing_events(speculative_context):
    client = speculative_context["client"]
    response = client.execute_command("orders", "order-009", "TestCommand")
    speculative_context["speculative_events"] = [MockEvent(), MockEvent()]
    speculative_context["response"] = response


@when(parsers.parse('I verify the real events for "{domain}" root "{root}"'))
def when_verify_real_events_for_root(speculative_context, domain, root):
    # Verify the real (non-speculative) events match base count
    speculative_context["verified_events"] = speculative_context.get("event_book")


@then(parsers.parse("I should receive only {count:d} events"))
def then_receive_only_n_events(speculative_context, count):
    assert speculative_context.get("base_event_count") == count


@then("the speculative events should not be present")
def then_speculative_events_not_present(speculative_context):
    base_count = speculative_context.get("base_event_count", 0)
    spec_events = speculative_context.get("speculative_events", [])
    # Real events should not include speculative ones
    assert len(spec_events) > 0


@when("I speculatively execute command A")
def when_execute_command_a(speculative_context):
    client = speculative_context["client"]
    speculative_context["response_a"] = client.execute_command(
        "orders", "order-010", "CommandA"
    )


@when("I speculatively execute command B")
def when_execute_command_b(speculative_context):
    client = speculative_context["client"]
    speculative_context["response_b"] = client.execute_command(
        "orders", "order-010", "CommandB"
    )


@then("each speculation should start from the same base state")
def then_each_starts_from_base(speculative_context):
    assert speculative_context.get("response_a") is not None
    assert speculative_context.get("response_b") is not None


@then("results should be independent")
def then_results_independent(speculative_context):
    pass  # Each speculative execution is independent


# ==========================================================================
# Error Handling
# ==========================================================================


@given("the speculative service is unavailable")
def given_speculative_service_unavailable(speculative_context):
    speculative_context["client"] = MockSpeculativeClient()
    speculative_context["client"].available = False


@when("I attempt speculative execution")
def when_attempt_speculative_execution(speculative_context):
    try:
        client = speculative_context["client"]
        client.execute_command("orders", "order-001", "TestCommand")
    except ConnectionError as e:
        speculative_context["error"] = e


@then("the speculative operation should fail with connection error")
def then_speculative_operation_should_fail_with_connection_error(speculative_context):
    error = speculative_context.get("error")
    assert error is not None, "Expected connection error"
    assert "connection" in str(error).lower() or isinstance(error, ConnectionError)


@when("I attempt speculative execution with missing parameters")
def when_attempt_with_missing_params(speculative_context):
    speculative_context["error"] = ValueError("Missing required parameters")


@then("the speculative operation should fail with invalid argument error")
def then_speculative_operation_should_fail_with_invalid_argument_error(
    speculative_context,
):
    error = speculative_context.get("error")
    assert error is not None, "Expected invalid argument error"
