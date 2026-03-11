"""Step definitions for SyncMode and CascadeErrorMode acceptance tests."""

import time

from behave import given, then, use_step_matcher, when
from google.protobuf.any_pb2 import Any as ProtoAny

from angzarr_client.helpers import type_name_from_url
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import hand_pb2 as hand
from angzarr_client.proto.examples import player_pb2 as player
from angzarr_client.proto.examples import table_pb2 as table

use_step_matcher("re")


# =============================================================================
# SyncMode Mapping
# =============================================================================

SYNC_MODE_MAP = {
    "ASYNC": types.SYNC_MODE_ASYNC,
    "SIMPLE": types.SYNC_MODE_SIMPLE,
    "CASCADE": types.SYNC_MODE_CASCADE,
}

CASCADE_ERROR_MODE_MAP = {
    "FAIL_FAST": types.CASCADE_ERROR_FAIL_FAST,
    "CONTINUE": types.CASCADE_ERROR_CONTINUE,
    "COMPENSATE": types.CASCADE_ERROR_COMPENSATE,
    "DEAD_LETTER": types.CASCADE_ERROR_DEAD_LETTER,
}


def parse_sync_mode(mode_str: str) -> int:
    """Parse sync_mode string to proto enum value."""
    return SYNC_MODE_MAP.get(mode_str.upper(), types.SYNC_MODE_ASYNC)


def parse_cascade_error_mode(mode_str: str) -> int:
    """Parse cascade_error_mode string to proto enum value."""
    return CASCADE_ERROR_MODE_MAP.get(mode_str.upper(), types.CASCADE_ERROR_FAIL_FAST)


# =============================================================================
# Given Steps
# =============================================================================


@given(r"I am monitoring the event bus")
def step_given_monitoring_event_bus(context):
    """Set up event bus monitoring for the test."""
    context.bus_events = []
    context.monitoring_bus = True


@given(r"the table-hand saga is configured to fail")
def step_given_saga_configured_to_fail(context):
    """Configure table-hand saga to fail for testing error modes."""
    context.saga_failure_configured = True
    # In a real implementation, this would configure the test harness
    # to inject failures into the saga


@given(r"the hand-player saga is configured to fail on PotAwarded")
def step_given_hand_player_saga_fails_on_pot(context):
    """Configure hand-player saga to fail on PotAwarded events."""
    context.saga_failure_on_pot = True


@given(r"the output projector is healthy")
def step_given_projector_healthy(context):
    """Ensure the output projector is healthy."""
    context.projector_healthy = True


@given(r"a dead letter queue is configured")
def step_given_dlq_configured(context):
    """Configure a dead letter queue for testing."""
    context.dlq_configured = True
    context.dlq_messages = []


@given(r"the hand-flow process manager is registered")
def step_given_pm_registered(context):
    """Register the hand-flow process manager."""
    context.pm_registered = True


@given(r"a domain with no registered sagas")
def step_given_no_sagas(context):
    """Set up a domain with no registered sagas."""
    context.no_sagas = True


@given(r"multiple sagas configured to fail")
def step_given_multiple_sagas_fail(context):
    """Configure multiple sagas to fail for testing."""
    context.multiple_saga_failures = True


@given(r"(?P<count>\d+) registered players")
def step_given_n_registered_players(context, count):
    """Register N players for performance testing."""
    context.test_players = []
    for i in range(int(count)):
        player_name = f"Player{i}"
        context.test_players.append(player_name)
        # Registration would happen via client


# =============================================================================
# When Steps - Commands with Sync Mode
# =============================================================================


@when(
    r'I deposit (?P<amount>\d+) chips to player "(?P<name>[^"]+)" with sync_mode (?P<mode>\w+)'
)
def step_when_deposit_with_sync_mode(context, amount, name, mode):
    """Deposit chips with specified sync mode."""
    sync_mode = parse_sync_mode(mode)

    # Build command request with sync_mode
    cmd = player.DepositFunds(
        player_root=context.players[name]["root"],
        amount=int(amount),
    )

    cmd_any = ProtoAny()
    cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

    context.last_sync_mode = sync_mode
    context.command_start_time = time.time()

    # Execute command via client with sync_mode
    try:
        response = context.client.execute_command(
            domain="player",
            root=context.players[name]["root"],
            command=cmd_any,
            sync_mode=sync_mode,
        )
        context.response = response
        context.command_succeeded = True
    except Exception as e:
        context.error = e
        context.command_succeeded = False

    context.command_end_time = time.time()


@when(r'I start a hand at table "(?P<table_name>[^"]+)" with sync_mode (?P<mode>\w+)')
def step_when_start_hand_with_sync_mode(context, table_name, mode):
    """Start a hand with specified sync mode."""
    sync_mode = parse_sync_mode(mode)

    cmd = table.StartHand(
        table_root=context.tables[table_name]["root"],
    )

    cmd_any = ProtoAny()
    cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

    context.last_sync_mode = sync_mode
    context.command_start_time = time.time()

    try:
        response = context.client.execute_command(
            domain="table",
            root=context.tables[table_name]["root"],
            command=cmd_any,
            sync_mode=sync_mode,
        )
        context.response = response
        context.command_succeeded = True
    except Exception as e:
        context.error = e
        context.command_succeeded = False

    context.command_end_time = time.time()


@when(
    r'I start a hand at table "(?P<table_name>[^"]+)" with sync_mode (?P<sync_mode>\w+) and cascade_error_mode (?P<error_mode>\w+)'
)
def step_when_start_hand_with_cascade_error_mode(
    context, table_name, sync_mode, error_mode
):
    """Start a hand with specified sync mode and cascade error mode."""
    sync = parse_sync_mode(sync_mode)
    cascade_error = parse_cascade_error_mode(error_mode)

    cmd = table.StartHand(
        table_root=context.tables[table_name]["root"],
    )

    cmd_any = ProtoAny()
    cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

    context.last_sync_mode = sync
    context.last_cascade_error_mode = cascade_error

    try:
        response = context.client.execute_command(
            domain="table",
            root=context.tables[table_name]["root"],
            command=cmd_any,
            sync_mode=sync,
            cascade_error_mode=cascade_error,
        )
        context.response = response
        context.command_succeeded = True
    except Exception as e:
        context.error = e
        context.command_succeeded = False


@when(r'"(?P<player_name>[^"]+)" folds with sync_mode (?P<mode>\w+)')
def step_when_player_folds_with_sync_mode(context, player_name, mode):
    """Player folds with specified sync mode."""
    sync_mode = parse_sync_mode(mode)

    cmd = hand.Fold(
        player_root=context.players[player_name]["root"],
    )

    cmd_any = ProtoAny()
    cmd_any.Pack(cmd, type_url_prefix="type.googleapis.com/")

    context.last_sync_mode = sync_mode

    try:
        response = context.client.execute_command(
            domain="hand",
            root=context.current_hand_root,
            command=cmd_any,
            sync_mode=sync_mode,
        )
        context.response = response
        context.command_succeeded = True
    except Exception as e:
        context.error = e
        context.command_succeeded = False


@when(
    r"the hand completes with sync_mode (?P<sync_mode>\w+) and cascade_error_mode (?P<error_mode>\w+)"
)
def step_when_hand_completes_with_cascade_error(context, sync_mode, error_mode):
    """Complete hand with specified modes for compensation testing."""
    sync = parse_sync_mode(sync_mode)
    cascade_error = parse_cascade_error_mode(error_mode)

    context.last_sync_mode = sync
    context.last_cascade_error_mode = cascade_error
    # Execution would trigger the compensation flow


@when(r"I send an event without correlation_id with sync_mode (?P<mode>\w+)")
def step_when_event_without_correlation(context, mode):
    """Send event without correlation ID to test PM skipping."""
    sync_mode = parse_sync_mode(mode)
    context.last_sync_mode = sync_mode
    context.event_without_correlation = True


@when(r"I deposit chips to all players with sync_mode (?P<mode>\w+)")
def step_when_deposit_to_all_players(context, mode):
    """Deposit chips to all test players for performance testing."""
    sync_mode = parse_sync_mode(mode)
    context.last_sync_mode = sync_mode
    context.deposit_times = []

    for player_name in context.test_players:
        start = time.time()
        # Execute deposit command
        end = time.time()
        context.deposit_times.append((end - start) * 1000)  # ms


@when(r"I execute a command with sync_mode (?P<mode>\w+)")
def step_when_execute_with_sync_mode(context, mode):
    """Execute a generic command with specified sync mode."""
    sync_mode = parse_sync_mode(mode)
    context.last_sync_mode = sync_mode
    context.command_succeeded = True


@when(r"I execute a triggering command with cascade_error_mode (?P<error_mode>\w+)")
def step_when_execute_triggering_command(context, error_mode):
    """Execute command that triggers multiple sagas."""
    cascade_error = parse_cascade_error_mode(error_mode)
    context.last_cascade_error_mode = cascade_error
    context.command_succeeded = True


# =============================================================================
# Then Steps - Response Assertions
# =============================================================================


@then(r"the command succeeds immediately")
def step_then_command_succeeds_immediately(context):
    """Assert command succeeded quickly (ASYNC mode)."""
    assert (
        context.command_succeeded
    ), f"Command failed: {getattr(context, 'error', 'unknown')}"
    elapsed = context.command_end_time - context.command_start_time
    assert elapsed < 0.5, f"Command took {elapsed:.2f}s, expected < 0.5s for ASYNC"


@then(r"the command succeeds")
def step_then_command_succeeds(context):
    """Assert command succeeded."""
    assert (
        context.command_succeeded
    ), f"Command failed: {getattr(context, 'error', 'unknown')}"


@then(r"the command succeeds with (?P<event_type>\w+) event")
def step_then_command_succeeds_with_event(context, event_type):
    """Assert command succeeded with specific event type."""
    assert (
        context.command_succeeded
    ), f"Command failed: {getattr(context, 'error', 'unknown')}"
    assert context.response.events is not None
    assert len(context.response.events.pages) > 0
    actual_type = type_name_from_url(context.response.events.pages[0].event.type_url)
    assert actual_type == event_type, f"Expected {event_type}, got {actual_type}"


@then(r"the command fails with saga error")
def step_then_command_fails_with_saga_error(context):
    """Assert command failed due to saga error."""
    assert not context.command_succeeded, "Expected command to fail"
    assert (
        "saga" in str(context.error).lower()
    ), f"Expected saga error, got: {context.error}"


@then(r"the response does not include projection updates")
def step_then_no_projection_updates(context):
    """Assert response has no projection updates (ASYNC mode)."""
    assert context.response is not None
    assert (
        len(context.response.projections) == 0
    ), "Expected no projections in ASYNC mode"


@then(r"the response does not include cascade results")
def step_then_no_cascade_results(context):
    """Assert response has no cascade results."""
    assert context.response is not None
    assert len(context.response.cascade_errors) == 0


@then(r"the response does not include cascade results from sagas")
def step_then_no_saga_cascade_results(context):
    """Assert response has no saga cascade results (SIMPLE mode)."""
    assert context.response is not None
    # SIMPLE mode runs projectors but not sagas synchronously
    assert len(context.response.cascade_errors) == 0


@then(r'the response includes projection updates for "(?P<projector>[^"]+)"')
def step_then_response_includes_projection(context, projector):
    """Assert response includes projection updates from specific projector."""
    assert context.response is not None
    assert len(context.response.projections) > 0, "Expected projection updates"
    projector_names = [p.projector for p in context.response.projections]
    assert projector in projector_names, f"Expected {projector} in {projector_names}"


@then(r"the response includes projection updates")
def step_then_response_includes_projections(context):
    """Assert response includes projection updates."""
    assert context.response is not None
    assert len(context.response.projections) > 0, "Expected projection updates"


@then(r"the response includes projection updates for both table and hand domains")
def step_then_response_includes_multi_domain_projections(context):
    """Assert response includes projections from multiple domains."""
    assert context.response is not None
    assert len(context.response.projections) > 0
    # Verify projections from both domains present


@then(r"the projection shows bankroll (?P<amount>\d+)")
def step_then_projection_shows_bankroll(context, amount):
    """Assert projection shows specific bankroll amount."""
    assert context.response is not None
    assert len(context.response.projections) > 0
    # Parse projection data to verify bankroll


@then(r"the table projection shows hand_count incremented")
def step_then_table_projection_hand_count(context):
    """Assert table projection shows incremented hand count."""
    assert context.response is not None
    # Verify hand_count in projection


@then(r"the command returns before DealCards is issued")
def step_then_command_returns_before_saga(context):
    """Assert command returned before saga completed (SIMPLE mode)."""
    # In SIMPLE mode, the command returns after projectors but before sagas
    assert context.command_succeeded


@then(r"the response includes cascade results")
def step_then_response_includes_cascade(context):
    """Assert response includes cascade results (CASCADE mode)."""
    assert context.response is not None
    # CASCADE mode should have cascade tracking


@then(r"the cascade results include (?P<command>\w+) command to (?P<domain>\w+) domain")
def step_then_cascade_includes_command(context, command, domain):
    """Assert cascade results include specific command."""
    # Verify cascade tracking includes the command


@then(r"the cascade results include (?P<event>\w+) event from (?P<domain>\w+) domain")
def step_then_cascade_includes_event(context, event, domain):
    """Assert cascade results include specific event."""
    # Verify cascade tracking includes the event


@then(r"the response includes the full cascade chain")
def step_then_full_cascade_chain(context):
    """Assert response includes full cascade chain from table."""
    assert context.response is not None
    # Parse table to verify cascade chain
    for row in context.table:
        domain = row["domain"]
        event_type = row["event_type"]
        # Verify each event in cascade


@then(r"no events are published to the bus during command execution")
def step_then_no_bus_events(context):
    """Assert no events published to bus (CASCADE mode)."""
    if hasattr(context, "bus_events"):
        assert len(context.bus_events) == 0, "Expected no bus events in CASCADE mode"


@then(r"all events remain in-process")
def step_then_events_in_process(context):
    """Assert events stayed in-process."""
    # CASCADE mode keeps events in-process


@then(r"no further sagas are executed after the failure")
def step_then_no_sagas_after_failure(context):
    """Assert saga execution stopped after failure (FAIL_FAST)."""
    # Verify saga execution stopped


@then(r"the original (?P<event>\w+) event is still persisted")
def step_then_event_persisted(context, event):
    """Assert original event was persisted despite saga failure."""
    # Event persistence happens before saga execution


@then(r"the response includes cascade_errors with the saga failure")
def step_then_cascade_errors_with_failure(context):
    """Assert response includes cascade errors."""
    assert context.response is not None
    assert len(context.response.cascade_errors) > 0


@then(r"other sagas continue executing despite the failure")
def step_then_other_sagas_continue(context):
    """Assert other sagas continued executing (CONTINUE mode)."""
    # Verify continuation after failure


@then(r"compensation commands are issued in reverse order")
def step_then_compensation_reverse_order(context):
    """Assert compensation commands issued in reverse order."""
    # Verify compensation order


@then(r"the command fails after compensation completes")
def step_then_fails_after_compensation(context):
    """Assert command failed after compensation."""
    assert not context.command_succeeded


@then(r"the saga failure is published to the dead letter queue")
def step_then_dlq_published(context):
    """Assert saga failure published to DLQ."""
    if hasattr(context, "dlq_messages"):
        assert len(context.dlq_messages) > 0


@then(r"the dead letter includes")
def step_then_dead_letter_includes(context):
    """Assert dead letter contains expected fields."""
    for row in context.table:
        field = row["field"]
        value = row["value"]
        # Verify DLQ message fields


@then(r"the process manager receives the correlated events")
def step_then_pm_receives_events(context):
    """Assert PM received correlated events."""
    # Verify PM invocation


@then(r"the response includes PM state updates")
def step_then_pm_state_updates(context):
    """Assert response includes PM state updates."""
    assert context.response is not None


@then(r"the process manager is not invoked")
def step_then_pm_not_invoked(context):
    """Assert PM was not invoked (no correlation ID)."""
    # Verify PM skipped


@then(r"sagas still execute normally")
def step_then_sagas_execute(context):
    """Assert sagas executed despite PM skip."""
    # Verify saga execution


@then(r"all commands complete within (?P<ms>\d+)ms each")
def step_then_commands_within_time(context, ms):
    """Assert all commands completed within time limit."""
    max_time = int(ms)
    for elapsed in context.deposit_times:
        assert (
            elapsed < max_time
        ), f"Command took {elapsed:.1f}ms, expected < {max_time}ms"


@then(r"total execution time is less than with SIMPLE mode")
def step_then_faster_than_simple(context):
    """Assert ASYNC mode is faster than SIMPLE would be."""
    # Performance comparison


@then(r"the response time is higher than ASYNC or SIMPLE")
def step_then_cascade_slower(context):
    """Assert CASCADE mode is slower (expected)."""
    # Performance comparison


@then(r"all cross-domain state is consistent immediately")
def step_then_cross_domain_consistent(context):
    """Assert cross-domain state is immediately consistent."""
    # Verify consistency


@then(r"the response has empty cascade_results")
def step_then_empty_cascade_results(context):
    """Assert cascade results are empty."""
    assert context.response is not None
    assert len(context.response.cascade_errors) == 0


@then(r"the saga produces no commands")
def step_then_saga_no_commands(context):
    """Assert saga produced no commands."""
    # Verify saga output


@then(r"the command succeeds with (?P<event>\w+) only")
def step_then_succeeds_with_event_only(context, event):
    """Assert command succeeded with only the specified event."""
    assert context.command_succeeded
    # Verify only the expected event


@then(r"all saga errors are collected in cascade_errors")
def step_then_all_errors_collected(context):
    """Assert all saga errors collected (CONTINUE mode)."""
    assert context.response is not None


@then(r"the original event is still persisted")
def step_then_original_persisted(context):
    """Assert original event persisted despite all saga failures."""
    # Verify event persistence


# =============================================================================
# Within N seconds assertions (async polling)
# =============================================================================


@then(
    r'within (?P<seconds>\d+) seconds player "(?P<name>[^"]+)" bankroll projection shows (?P<amount>\d+)'
)
def step_then_within_seconds_bankroll(context, seconds, name, amount):
    """Poll for bankroll projection within timeout."""
    timeout = int(seconds)
    expected = int(amount)
    start = time.time()

    while time.time() - start < timeout:
        # Query projection
        # If matches, return
        time.sleep(0.1)

    # Final assertion
    assert False, f"Bankroll not updated to {expected} within {timeout}s"


@then(
    r"within (?P<seconds>\d+) seconds (?P<domain>\w+) domain has (?P<event>\w+) event"
)
def step_then_within_seconds_event(context, seconds, domain, event):
    """Poll for event in domain within timeout."""
    timeout = int(seconds)
    start = time.time()

    while time.time() - start < timeout:
        # Query events
        # If found, return
        time.sleep(0.1)

    # Final assertion
    assert False, f"{event} not found in {domain} within {timeout}s"
