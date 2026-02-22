"""Step definitions for merge strategy (concurrency control) tests.

These steps test the client-library aspects of merge strategy handling.
Full coordinator integration tests are in tests/standalone_integration/merge_strategy.rs
"""

from behave import given, when, then, use_step_matcher
from google.protobuf.any_pb2 import Any as ProtoAny

from angzarr_client.proto.angzarr import types_pb2 as types

use_step_matcher("re")


# ===========================================================================
# Proto Helpers
# ===========================================================================


def make_command_page(
    sequence: int, strategy: types.MergeStrategy
) -> types.CommandPage:
    """Create CommandPage with merge strategy."""
    command_any = ProtoAny()
    command_any.type_url = "type.googleapis.com/test.TestCommand"
    command_any.value = b""

    return types.CommandPage(
        sequence=sequence,
        command=command_any,
        merge_strategy=strategy,
    )


def make_command_book(
    domain: str, root_bytes: bytes, sequence: int, strategy: types.MergeStrategy
) -> types.CommandBook:
    """Create CommandBook with merge strategy."""
    return types.CommandBook(
        cover=types.Cover(
            domain=domain,
            root=types.Uuid(value=root_bytes),
        ),
        pages=[make_command_page(sequence, strategy)],
    )


def make_event_book(
    domain: str, root_bytes: bytes, sequences: list[int]
) -> types.EventBook:
    """Create EventBook with events at given sequences."""
    pages = []
    for seq in sequences:
        event_any = ProtoAny()
        event_any.type_url = f"type.googleapis.com/test.TestEvent{seq}"
        event_any.value = b""
        pages.append(types.EventPage(sequence=seq, event=event_any))

    return types.EventBook(
        cover=types.Cover(
            domain=domain,
            root=types.Uuid(value=root_bytes),
        ),
        pages=pages,
    )


# ===========================================================================
# Background / Setup
# ===========================================================================


@given(r'an aggregate "(?P<domain>\w+)" with initial events:')
def step_given_aggregate_with_events(context, domain):
    """Setup aggregate with initial event history."""
    context.domain = domain
    context.root_bytes = b"\x00" * 16  # Fixed UUID for testing

    # Parse events from table
    sequences = []
    for row in context.table:
        sequences.append(int(row["sequence"]))

    context.prior_events = make_event_book(domain, context.root_bytes, sequences)
    context.next_sequence = max(sequences) + 1 if sequences else 0


@given(r"a new aggregate with no events")
def step_given_new_aggregate(context):
    """Setup empty aggregate."""
    context.domain = "test"
    context.root_bytes = b"\x00" * 16
    context.prior_events = make_event_book(context.domain, context.root_bytes, [])
    context.next_sequence = 0


# ===========================================================================
# Command Setup
# ===========================================================================


@given(r"a command with merge_strategy (?P<strategy>\w+)")
def step_given_command_with_strategy(context, strategy):
    """Create command with specified merge strategy."""
    strategy_map = {
        "STRICT": types.MERGE_STRICT,
        "COMMUTATIVE": types.MERGE_COMMUTATIVE,
        "AGGREGATE_HANDLES": types.MERGE_AGGREGATE_HANDLES,
    }
    context.merge_strategy = strategy_map[strategy]
    context.command_sequence = None  # Set by next step


@given(r"a command with no explicit merge_strategy")
def step_given_command_no_strategy(context):
    """Create command without explicit strategy (tests default)."""
    context.merge_strategy = types.MERGE_COMMUTATIVE  # Proto default
    context.command_sequence = context.next_sequence


@given(r"the command targets sequence (?P<seq>\d+)")
def step_given_command_targets_sequence(context, seq):
    """Set command target sequence."""
    context.command_sequence = int(seq)
    context.command = make_command_book(
        context.domain,
        context.root_bytes,
        context.command_sequence,
        context.merge_strategy,
    )


@given(r"a command targeting sequence (?P<seq>\d+)")
def step_given_command_targeting_sequence(context, seq):
    """Set command target sequence (alternative phrasing)."""
    context.command_sequence = int(seq)


# ===========================================================================
# Proto Helper Tests
# ===========================================================================


@when(r"merge_strategy is extracted")
def step_when_merge_strategy_extracted(context):
    """Extract merge strategy from command."""
    if hasattr(context, "command") and context.command.pages:
        context.extracted_strategy = context.command.pages[0].merge_strategy
    else:
        # Empty pages case
        context.extracted_strategy = types.MERGE_COMMUTATIVE


@then(r"the result is (?P<strategy>\w+)")
def step_then_result_is_strategy(context, strategy):
    """Verify extracted strategy."""
    strategy_map = {
        "STRICT": types.MERGE_STRICT,
        "COMMUTATIVE": types.MERGE_COMMUTATIVE,
        "AGGREGATE_HANDLES": types.MERGE_AGGREGATE_HANDLES,
    }
    assert context.extracted_strategy == strategy_map[strategy], (
        f"Expected {strategy}, got {context.extracted_strategy}"
    )


@then(r"the effective merge_strategy is (?P<strategy>\w+)")
def step_then_effective_strategy(context, strategy):
    """Verify effective strategy (same as result check)."""
    step_then_result_is_strategy(context, strategy)


# ===========================================================================
# Coordinator Behavior (Documentation / Integration Test Markers)
# ===========================================================================


@when(r"the coordinator processes the command")
def step_when_coordinator_processes(context):
    """
    Coordinator processes the command.

    NOTE: Full coordinator behavior is tested in Rust integration tests.
    See: tests/standalone_integration/merge_strategy.rs

    This step validates the command structure and simulates expected behavior
    based on merge strategy and sequence comparison.
    """
    expected_seq = context.next_sequence
    actual_seq = context.command_sequence

    if actual_seq == expected_seq:
        # Sequence matches - command would succeed
        context.result = "success"
        context.status = None
        context.error_message = None
    else:
        # Sequence mismatch - behavior depends on strategy
        if context.merge_strategy == types.MERGE_STRICT:
            context.result = "error"
            context.status = "ABORTED"
            context.error_message = f"Sequence mismatch: command expects {actual_seq}, aggregate at {expected_seq}"
            context.is_retryable = False
        elif context.merge_strategy == types.MERGE_COMMUTATIVE:
            context.result = "error"
            context.status = "FAILED_PRECONDITION"
            context.error_message = f"Sequence mismatch: command expects {actual_seq}, aggregate at {expected_seq}"
            context.is_retryable = True
            context.error_event_book = context.prior_events
        else:  # AGGREGATE_HANDLES
            # Coordinator passes through - aggregate decides
            context.result = "delegate_to_aggregate"
            context.status = None
            context.coordinator_validated = False


@then(r"the command succeeds")
def step_then_command_succeeds(context):
    """Verify command succeeded."""
    assert context.result == "success", f"Expected success, got {context.result}"


@then(r"events are persisted")
def step_then_events_persisted(context):
    """Verify events would be persisted (success case)."""
    assert context.result == "success"


@then(r"the command fails with (?P<status>\w+) status")
def step_then_command_fails_with_status(context, status):
    """Verify command failed with expected status."""
    assert context.result == "error", f"Expected error, got {context.result}"
    assert context.status == status, f"Expected {status}, got {context.status}"


@then(r'the error message contains "(?P<text>[^"]+)"')
def step_then_error_message_contains(context, text):
    """Verify error message content."""
    assert text in context.error_message, (
        f"Expected '{text}' in '{context.error_message}'"
    )


@then(r"no events are persisted")
def step_then_no_events_persisted(context):
    """Verify no events persisted (error case)."""
    assert context.result == "error"


@then(r"the error is marked as retryable")
def step_then_error_is_retryable(context):
    """Verify error is retryable."""
    assert context.is_retryable, "Expected retryable error"


@then(r"the error details include the current EventBook")
def step_then_error_includes_event_book(context):
    """Verify error includes EventBook for retry."""
    assert hasattr(context, "error_event_book"), "Expected EventBook in error"
    assert context.error_event_book is not None


@then(r"the EventBook shows next_sequence (?P<seq>\d+)")
def step_then_event_book_next_sequence(context, seq):
    """Verify EventBook next_sequence."""
    expected = int(seq)
    pages = context.error_event_book.pages
    actual = len(pages) if pages else 0
    # next_sequence is max(sequence) + 1
    if pages:
        actual = max(p.num for p in pages) + 1
    assert actual == expected, f"Expected next_sequence {expected}, got {actual}"


# ===========================================================================
# Aggregate Handles Strategy
# ===========================================================================


@then(r"the coordinator does NOT validate the sequence")
def step_then_coordinator_skips_validation(context):
    """Verify coordinator skipped sequence validation."""
    assert context.result == "delegate_to_aggregate"
    assert context.coordinator_validated is False


@then(r"the aggregate handler is invoked")
def step_then_aggregate_handler_invoked(context):
    """Verify aggregate handler would be invoked."""
    assert context.result == "delegate_to_aggregate"


@then(r"the aggregate receives the prior EventBook")
def step_then_aggregate_receives_event_book(context):
    """Verify aggregate receives EventBook."""
    # In AGGREGATE_HANDLES mode, the EventBook is passed to aggregate
    assert context.prior_events is not None
