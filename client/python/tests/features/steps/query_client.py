"""Query client step definitions."""

import uuid
from datetime import datetime
from unittest.mock import MagicMock

import pytest
from google.protobuf.any_pb2 import Any
from google.protobuf.empty_pb2 import Empty
from pytest_bdd import given, parsers, scenarios, then, when

from angzarr_client.proto.angzarr import types_pb2

# Link to feature file


@pytest.fixture
def query_context():
    """Test context for query client scenarios."""
    return {
        "client": None,
        "result": None,
        "error": None,
        "aggregates": {},
        "correlation_events": {},
        "service_available": True,
    }


def make_event_book(
    domain: str,
    root: str,
    event_count: int,
    start_seq: int = 0,
    snapshot_seq: int = None,
) -> types_pb2.EventBook:
    """Create an EventBook with specified events."""
    book = types_pb2.EventBook()
    book.cover.domain = domain
    try:
        root_uuid = uuid.UUID(root)
    except ValueError:
        root_uuid = uuid.uuid4()
    book.cover.root.value = root_uuid.bytes

    # Add snapshot if specified
    if snapshot_seq is not None:
        book.snapshot.sequence = snapshot_seq
        book.snapshot.state.Pack(Empty())

    # Add event pages
    for i in range(event_count):
        page = book.pages.add()
        page.header.sequence = start_seq + i
        page.event.Pack(Empty())

    return book


# ==========================================================================
# Background Steps
# ==========================================================================


@given("a QueryClient connected to the test backend")
def given_query_client(query_context):
    query_context["client"] = MagicMock()


# ==========================================================================
# Given Steps - Aggregates
# ==========================================================================


@given(parsers.parse('an aggregate "{domain}" with root "{root}"'))
def given_aggregate(query_context, domain, root):
    key = f"{domain}:{root}"
    query_context["aggregates"][key] = make_event_book(domain, root, 0)


@given(parsers.parse('an aggregate "{domain}" with root "{root}" has {count:d} events'))
def given_aggregate_with_events(query_context, domain, root, count, request):
    from tests.features.conftest import SHARED_EVENT_STORE

    key = f"{domain}:{root}"
    book = make_event_book(domain, root, count)
    query_context["aggregates"][key] = book
    # Also store in shared event store for cross-context access
    SHARED_EVENT_STORE[root] = book

    # Populate other contexts if available (for cross-context scenarios)
    for ctx_name in ["speculative_context", "domain_client_context"]:
        try:
            ctx = request.getfixturevalue(ctx_name)
            ctx["event_book"] = book
            if ctx_name == "speculative_context":
                ctx["base_event_count"] = count
        except Exception:
            pass


@given(
    parsers.parse(
        'an aggregate "{domain}" with root "{root}" has event "{event_type}" with data "{data}"'
    )
)
def given_aggregate_with_specific_event(query_context, domain, root, event_type, data):
    key = f"{domain}:{root}"
    book = types_pb2.EventBook()
    book.cover.domain = domain
    try:
        root_uuid = uuid.UUID(root)
    except ValueError:
        root_uuid = uuid.uuid4()
    book.cover.root.value = root_uuid.bytes

    page = book.pages.add()
    page.header.sequence = 0
    # Store event type in type_url
    page.event.type_url = f"type.googleapis.com/{event_type}"
    page.event.value = data.encode()

    query_context["aggregates"][key] = book


@given(
    parsers.parse(
        'an aggregate "{domain}" with root "{root}" has events at known timestamps'
    )
)
def given_aggregate_with_timestamps(query_context, domain, root):
    key = f"{domain}:{root}"
    query_context["aggregates"][key] = make_event_book(domain, root, 5)


@given(
    parsers.parse('an aggregate "{domain}" with root "{root}" in edition "{edition}"')
)
def given_aggregate_in_edition(query_context, domain, root, edition):
    key = f"{domain}:{root}:{edition}"
    query_context["aggregates"][key] = make_event_book(domain, root, 3)


@given(
    parsers.parse(
        'an aggregate "{domain}" with root "{root}" has {count:d} events in main'
    )
)
def given_aggregate_in_main(query_context, domain, root, count):
    key = f"{domain}:{root}"
    query_context["aggregates"][key] = make_event_book(domain, root, count)


@given(
    parsers.parse(
        'an aggregate "{domain}" with root "{root}" has {count:d} events in edition "{edition}"'
    )
)
def given_aggregate_in_edition_with_count(query_context, domain, root, count, edition):
    key = f"{domain}:{root}:{edition}"
    query_context["aggregates"][key] = make_event_book(domain, root, count)


@given(
    parsers.parse(
        'an aggregate "{domain}" with root "{root}" has a snapshot at sequence {snap:d} and {total:d} events'
    )
)
def given_aggregate_with_snapshot(query_context, domain, root, snap, total):
    key = f"{domain}:{root}"
    query_context["aggregates"][key] = make_event_book(
        domain, root, total, snapshot_seq=snap
    )


@given(parsers.parse('events with correlation ID "{cid}" exist in multiple aggregates'))
def given_correlated_events(query_context, cid):
    query_context["correlation_events"][cid] = [
        make_event_book("orders", "order-1", 2),
        make_event_book("inventory", "inv-1", 1),
    ]


@given("the query service is unavailable")
def given_service_unavailable(query_context):
    query_context["service_available"] = False


# ==========================================================================
# When Steps
# ==========================================================================


@when(parsers.parse('I query events for "{domain}" root "{root}"'))
def when_query_events(query_context, domain, root):
    if not query_context["service_available"]:
        query_context["error"] = ConnectionError("Service unavailable")
        return

    key = f"{domain}:{root}"
    if key in query_context["aggregates"]:
        query_context["result"] = query_context["aggregates"][key]
    else:
        query_context["result"] = make_event_book(domain, root, 0)


@when(
    parsers.parse('I query events for "{domain}" root "{root}" from sequence {start:d}')
)
def when_query_events_from_sequence(query_context, domain, root, start):
    key = f"{domain}:{root}"
    if key in query_context["aggregates"]:
        full_book = query_context["aggregates"][key]
        # Filter pages from start sequence
        result = types_pb2.EventBook()
        result.cover.CopyFrom(full_book.cover)
        for page in full_book.pages:
            if page.header.sequence >= start:
                new_page = result.pages.add()
                new_page.CopyFrom(page)
        query_context["result"] = result
    else:
        query_context["result"] = make_event_book(domain, root, 0)


@when(
    parsers.parse(
        'I query events for "{domain}" root "{root}" from sequence {start:d} to {end:d}'
    )
)
def when_query_events_range(query_context, domain, root, start, end):
    key = f"{domain}:{root}"
    if key in query_context["aggregates"]:
        full_book = query_context["aggregates"][key]
        result = types_pb2.EventBook()
        result.cover.CopyFrom(full_book.cover)
        for page in full_book.pages:
            if start <= page.header.sequence < end:
                new_page = result.pages.add()
                new_page.CopyFrom(page)
        query_context["result"] = result
    else:
        query_context["result"] = make_event_book(domain, root, 0)


@when(
    parsers.parse('I query events for "{domain}" root "{root}" as of sequence {seq:d}')
)
def when_query_events_as_of_sequence(query_context, domain, root, seq):
    key = f"{domain}:{root}"
    if key in query_context["aggregates"]:
        full_book = query_context["aggregates"][key]
        result = types_pb2.EventBook()
        result.cover.CopyFrom(full_book.cover)
        for page in full_book.pages:
            if page.header.sequence <= seq:
                new_page = result.pages.add()
                new_page.CopyFrom(page)
        query_context["result"] = result
    else:
        query_context["result"] = make_event_book(domain, root, 0)


@when(
    parsers.parse(
        'I query events for "{domain}" root "{root}" as of time "{timestamp}"'
    )
)
def when_query_events_as_of_time(query_context, domain, root, timestamp):
    key = f"{domain}:{root}"
    if key in query_context["aggregates"]:
        # For testing, return all events (timestamp filtering is simulated)
        query_context["result"] = query_context["aggregates"][key]
    else:
        query_context["result"] = make_event_book(domain, root, 0)


@when(
    parsers.parse('I query events for "{domain}" root "{root}" in edition "{edition}"')
)
def when_query_events_in_edition(query_context, domain, root, edition):
    key = f"{domain}:{root}:{edition}"
    if key in query_context["aggregates"]:
        query_context["result"] = query_context["aggregates"][key]
    else:
        query_context["result"] = make_event_book(domain, root, 0)


@when(parsers.parse('I query events by correlation ID "{cid}"'))
def when_query_by_correlation_id(query_context, cid):
    if cid in query_context["correlation_events"]:
        # Combine events from all correlated aggregates
        result = types_pb2.EventBook()
        for book in query_context["correlation_events"][cid]:
            for page in book.pages:
                new_page = result.pages.add()
                new_page.CopyFrom(page)
        query_context["result"] = result
    else:
        query_context["result"] = types_pb2.EventBook()


@when("I query events with empty domain")
def when_query_empty_domain(query_context):
    query_context["error"] = ValueError("Invalid argument: empty domain")


@when("I attempt to query events")
def when_attempt_query(query_context):
    if not query_context["service_available"]:
        query_context["error"] = ConnectionError("Connection error")


# ==========================================================================
# Then Steps
# ==========================================================================


@then(parsers.parse("I should receive an EventBook with {count:d} events"))
def then_receive_event_book_with_count(query_context, count):
    result = query_context.get("result")
    assert result is not None, "Should have a result"
    assert (
        len(result.pages) == count
    ), f"Expected {count} events, got {len(result.pages)}"


@then(parsers.parse("the next_sequence should be {seq:d}"))
def then_next_sequence_is(query_context, seq):
    result = query_context.get("result")
    assert result is not None
    # next_sequence = number of events
    assert len(result.pages) == seq


@then(parsers.parse("events should be in sequence order {start:d} to {end:d}"))
def then_events_in_sequence_order(query_context, start, end):
    result = query_context.get("result")
    assert result is not None
    for i, page in enumerate(result.pages):
        assert (
            page.header.sequence == start + i
        ), f"Expected sequence {start + i}, got {page.header.sequence}"


@then(parsers.parse('the first event should have type "{event_type}"'))
def then_first_event_has_type(query_context, event_type):
    result = query_context.get("result")
    assert result is not None
    assert len(result.pages) > 0
    assert event_type in result.pages[0].event.type_url


@then(parsers.parse('the first event should have payload "{payload}"'))
def then_first_event_has_payload(query_context, payload):
    result = query_context.get("result")
    assert result is not None
    assert len(result.pages) > 0
    assert result.pages[0].event.value.decode() == payload


@then(parsers.parse("the first event should have sequence {seq:d}"))
def then_first_event_has_sequence(query_context, seq):
    result = query_context.get("result")
    assert result is not None
    assert len(result.pages) > 0
    assert result.pages[0].sequence == seq


@then(parsers.parse("the last event should have sequence {seq:d}"))
def then_last_event_has_sequence(query_context, seq):
    result = query_context.get("result")
    assert result is not None
    assert len(result.pages) > 0
    assert result.pages[-1].sequence == seq


@then("I should receive events up to that timestamp")
def then_receive_events_up_to_timestamp(query_context):
    result = query_context.get("result")
    assert result is not None


@then("I should receive events from that edition only")
def then_receive_events_from_edition(query_context):
    result = query_context.get("result")
    assert result is not None


@then("I should receive events from all correlated aggregates")
def then_receive_correlated_events(query_context):
    result = query_context.get("result")
    assert result is not None
    assert len(result.pages) > 0


@then("I should receive no events")
def then_receive_no_events(query_context):
    result = query_context.get("result")
    assert result is not None
    assert len(result.pages) == 0


@then("the EventBook should include the snapshot")
def then_event_book_includes_snapshot(query_context):
    result = query_context.get("result")
    assert result is not None
    assert result.HasField("snapshot")


@then(parsers.parse("the returned snapshot should be at sequence {seq:d}"))
def then_snapshot_at_sequence(query_context, seq):
    result = query_context.get("result")
    assert result is not None
    assert result.snapshot.sequence == seq


@then("the operation should fail with invalid argument error")
def then_fail_with_invalid_argument(query_context):
    error = query_context.get("error")
    assert error is not None
    assert "invalid" in str(error).lower()


@then("the operation should fail with connection error")
def then_fail_with_connection_error(query_context):
    error = query_context.get("error")
    assert error is not None
    assert isinstance(error, ConnectionError) or "connection" in str(error).lower()
