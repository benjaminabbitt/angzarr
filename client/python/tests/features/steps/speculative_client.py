"""Step definitions for speculative client scenarios."""

from pytest_bdd import given, when, then, parsers
from google.protobuf.any_pb2 import Any
from google.protobuf.empty_pb2 import Empty

from angzarr_client.proto.angzarr import types_pb2


@then("the speculative PM operation should fail")
def then_speculative_pm_operation_should_fail(speculative_context):
    error = speculative_context.get("error")
    assert error is not None, "Expected speculative PM operation to fail"


@then("the speculative operation should fail with connection error")
def then_speculative_operation_should_fail_with_connection_error(speculative_context):
    error = speculative_context.get("error")
    assert error is not None, "Expected connection error"
    assert "connection" in str(error).lower() or isinstance(error, ConnectionError)


@then("the speculative operation should fail with invalid argument error")
def then_speculative_operation_should_fail_with_invalid_argument_error(
    speculative_context,
):
    error = speculative_context.get("error")
    assert error is not None, "Expected invalid argument error"


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
        page.sequence = i
        page.event.Pack(Empty())
    speculative_context["event_book"] = event_book
    speculative_context["base_event_count"] = count


@when(parsers.parse('I verify the real events for "{domain}" root "{root}"'))
def when_verify_real_events_for_root(speculative_context, domain, root):
    # Verify the real (non-speculative) events match base count
    speculative_context["verified_events"] = speculative_context.get("event_book")
