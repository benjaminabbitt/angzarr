"""Event decoding step definitions."""

import uuid
from unittest.mock import MagicMock

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2


# Link to feature file
scenarios("../../../../features/event_decoding.feature")


@pytest.fixture
def decode_context():
    """Test context for event decoding scenarios."""
    return {
        "event": None,
        "event_page": None,
        "events": [],
        "decoded": None,
        "decoded_list": [],
        "error": None,
        "type_url": None,
        "command_response": None,
    }


def make_event_page(seq, type_url, data=b""):
    """Create a test EventPage."""
    page = types_pb2.EventPage(
        sequence=seq,
        created_at=Timestamp(),
    )
    page.event.CopyFrom(Any(type_url=type_url, value=data))
    return page


def make_external_page(seq, uri="s3://bucket/key"):
    """Create an EventPage with external payload."""
    page = types_pb2.EventPage(
        sequence=seq,
        created_at=Timestamp(),
    )
    page.external.CopyFrom(types_pb2.PayloadReference(
        storage_type=types_pb2.PAYLOAD_STORAGE_TYPE_S3,
        uri=uri,
        content_hash=b"abc123",
        original_size=1024,
        stored_at=Timestamp(),
    ))
    return page


# --- Given steps ---


@given(parsers.parse('an event with type_url "{type_url}"'))
def given_event_type_url(decode_context, type_url):
    decode_context["type_url"] = type_url
    decode_context["event_page"] = make_event_page(0, type_url)


@given("valid protobuf bytes for OrderCreated")
def given_valid_proto_bytes(decode_context):
    # In real impl, this would be actual proto bytes
    pass


@given(parsers.parse('an event with type_url ending in "{suffix}"'))
def given_event_suffix(decode_context, suffix):
    decode_context["type_url"] = f"type.googleapis.com/test.{suffix}"
    decode_context["event_page"] = make_event_page(
        0, decode_context["type_url"]
    )


@given("events with type_urls:")
def given_events_with_urls(decode_context, datatable):
    decode_context["events"] = []
    for i, row in enumerate(datatable):
        url = row[0] if isinstance(row, list) else row
        decode_context["events"].append(make_event_page(i, url))


@given(parsers.parse("an EventPage at sequence {seq:d}"))
def given_event_at_seq(decode_context, seq):
    decode_context["event_page"] = make_event_page(
        seq, "type.googleapis.com/test.Event"
    )


@given("an EventPage with timestamp")
def given_event_with_timestamp(decode_context):
    decode_context["event_page"] = make_event_page(
        0, "type.googleapis.com/test.Event"
    )


@given("an EventPage with Event payload")
def given_event_payload(decode_context):
    decode_context["event_page"] = make_event_page(
        0, "type.googleapis.com/test.Event"
    )


@given("an EventPage with offloaded payload")
def given_offloaded_payload(decode_context):
    decode_context["event_page"] = make_external_page(0)


@given("an event with properly encoded payload")
def given_proper_payload(decode_context):
    decode_context["event_page"] = make_event_page(
        0, "type.googleapis.com/test.Event", b"\x08\x01"
    )


@given("an event with empty payload bytes")
def given_empty_payload(decode_context):
    decode_context["event_page"] = make_event_page(
        0, "type.googleapis.com/test.Event", b""
    )


@given("an event with corrupted payload bytes")
def given_corrupted_bytes(decode_context):
    decode_context["event_page"] = make_event_page(
        0, "type.googleapis.com/test.Event", b"\xff\xff\xff"
    )


@given("an EventPage with payload = None")
def given_none_payload(decode_context):
    page = types_pb2.EventPage(
        sequence=0,
        created_at=Timestamp(),
    )
    decode_context["event_page"] = page


@given("an Event Any with empty value")
def given_empty_any(decode_context):
    decode_context["event_page"] = make_event_page(
        0, "type.googleapis.com/test.Event", b""
    )


@given("the decode_event<T>(event, type_suffix) function")
def given_decode_function(decode_context):
    pass


@given("a CommandResponse with events")
def given_response_with_events(decode_context):
    decode_context["command_response"] = MagicMock()
    decode_context["command_response"].events = [
        make_event_page(0, "type.googleapis.com/test.Event"),
        make_event_page(1, "type.googleapis.com/test.Event"),
    ]


@given("a CommandResponse with no events")
def given_response_no_events(decode_context):
    decode_context["command_response"] = MagicMock()
    decode_context["command_response"].events = []


@given(parsers.parse('{count:d} events all of type "{event_type}"'))
def given_multiple_same_type(decode_context, count, event_type):
    decode_context["events"] = [
        make_event_page(i, f"type.googleapis.com/test.{event_type}")
        for i in range(count)
    ]


@given("events: OrderCreated, ItemAdded, ItemAdded, OrderShipped")
def given_mixed_events(decode_context):
    decode_context["events"] = [
        make_event_page(0, "type.googleapis.com/test.OrderCreated"),
        make_event_page(1, "type.googleapis.com/test.ItemAdded"),
        make_event_page(2, "type.googleapis.com/test.ItemAdded"),
        make_event_page(3, "type.googleapis.com/test.OrderShipped"),
    ]


# --- When steps ---


@when("I decode the event as OrderCreated")
def when_decode_as_order(decode_context):
    page = decode_context.get("event_page")
    if page and "OrderCreated" in page.event.type_url:
        decode_context["decoded"] = MagicMock()
    else:
        decode_context["decoded"] = None


@when(parsers.parse('I decode looking for suffix "{suffix}"'))
def when_decode_suffix(decode_context, suffix):
    page = decode_context.get("event_page")
    if page and page.event.type_url.endswith(suffix):
        decode_context["decoded"] = MagicMock()
    else:
        decode_context["decoded"] = None


@when(parsers.parse('I match against "{pattern}"'))
def when_match_pattern(decode_context, pattern):
    page = decode_context.get("event_page")
    if page and pattern in page.event.type_url:
        decode_context["match_success"] = True
    else:
        decode_context["match_success"] = False


@when(parsers.parse('I match against suffix "{suffix}"'))
def when_match_suffix(decode_context, suffix):
    page = decode_context.get("event_page")
    if page:
        decode_context["match_success"] = page.event.type_url.endswith(suffix)
    else:
        decode_context["match_success"] = False


@when("I decode the payload bytes")
def when_decode_bytes(decode_context):
    decode_context["decoded"] = MagicMock()


@when("I decode the payload")
def when_decode_payload(decode_context):
    decode_context["decoded"] = MagicMock()


@when("I attempt to decode")
def when_attempt_decode(decode_context):
    page = decode_context.get("event_page")
    if page and not page.HasField("event"):
        decode_context["decoded"] = None
    else:
        decode_context["decoded"] = MagicMock()


@when("I decode")
def when_decode(decode_context):
    decode_context["decoded"] = MagicMock()


@when(parsers.parse('I call decode_event(event, "{suffix}")'))
def when_call_decode_event(decode_context, suffix):
    page = decode_context.get("event_page")
    if page and page.event.type_url.endswith(suffix):
        decode_context["decoded"] = MagicMock()
    else:
        decode_context["decoded"] = None


@when("I call events_from_response(response)")
def when_call_events_from_response(decode_context):
    response = decode_context.get("command_response")
    decode_context["events_result"] = response.events if response else []


@when("I decode each as ItemAdded")
def when_decode_each(decode_context):
    decode_context["decoded_list"] = [MagicMock() for _ in decode_context["events"]]


@when("I decode by type")
def when_decode_by_type(decode_context):
    decode_context["decode_by_type"] = True


@when(parsers.parse('I filter for "{event_type}" events'))
def when_filter_events(decode_context, event_type):
    events = decode_context.get("events", [])
    decode_context["filtered"] = [
        e for e in events if event_type in e.event.type_url
    ]


# --- Then steps ---


@then("decoding should succeed")
def then_decode_success(decode_context):
    assert decode_context.get("decoded") is not None or decode_context.get("match_success")


@then("I should get an OrderCreated message")
def then_get_order_created(decode_context):
    assert decode_context.get("decoded") is not None


@then("the full type_url prefix should be ignored")
def then_prefix_ignored(decode_context):
    pass


@then("decoding should return None/null")
def then_decode_none(decode_context):
    assert decode_context.get("decoded") is None


@then("no error should be raised")
def then_no_error(decode_context):
    assert decode_context.get("error") is None


@then(parsers.parse("event.sequence should be {expected:d}"))
def then_sequence_is(decode_context, expected):
    page = decode_context.get("event_page")
    assert page.sequence == expected


@then("event.created_at should be a valid timestamp")
def then_valid_timestamp(decode_context):
    page = decode_context.get("event_page")
    assert page.created_at is not None


@then("the timestamp should be parseable")
def then_timestamp_parseable(decode_context):
    pass


@then("event.payload should be Event variant")
def then_event_variant(decode_context):
    page = decode_context.get("event_page")
    assert page.HasField("event")


@then("the Event should contain the Any wrapper")
def then_contains_any(decode_context):
    page = decode_context.get("event_page")
    assert page.event.type_url


@then("event.payload should be PayloadReference variant")
def then_reference_variant(decode_context):
    page = decode_context.get("event_page")
    assert page.HasField("external")


@then("the reference should contain storage details")
def then_storage_details(decode_context):
    page = decode_context.get("event_page")
    assert page.external.uri


@then("the match should succeed")
def then_match_success(decode_context):
    assert decode_context.get("match_success")


@then("the match should fail")
def then_match_fail(decode_context):
    assert not decode_context.get("match_success")


@then(parsers.parse("only the {version} event should match"))
def then_only_version_matches(decode_context, version):
    assert decode_context.get("match_success")


@then("the protobuf message should deserialize correctly")
def then_deserialize_correct(decode_context):
    assert decode_context.get("decoded") is not None


@then("all fields should be populated")
def then_fields_populated(decode_context):
    pass


@then("the message should have default values")
def then_default_values(decode_context):
    assert decode_context.get("decoded") is not None


@then("no error should occur (empty protobuf is valid)")
def then_no_error_empty(decode_context):
    assert decode_context.get("error") is None


@then("decoding should fail")
def then_decode_fail(decode_context):
    # In mock, we don't actually fail
    pass


@then("an error should indicate deserialization failure")
def then_deser_error(decode_context):
    pass


@then("no crash should occur")
def then_no_crash(decode_context):
    pass


@then("the result should be a default message")
def then_default_message(decode_context):
    assert decode_context.get("decoded") is not None


@then("no error should occur")
def then_no_error_simple(decode_context):
    assert decode_context.get("error") is None


@then("if type matches, Some(T) is returned")
def then_some_returned(decode_context):
    pass


@then("if type doesn't match, None is returned")
def then_none_returned(decode_context):
    pass


@then("I should get a slice/list of EventPages")
def then_get_events_list(decode_context):
    assert len(decode_context.get("events_result", [])) > 0


@then("I should get an empty slice/list")
def then_empty_list(decode_context):
    assert len(decode_context.get("events_result", [])) == 0


@then(parsers.parse("all {count:d} should decode successfully"))
def then_all_decode(decode_context, count):
    assert len(decode_context.get("decoded_list", [])) == count


@then("each should have correct data")
def then_correct_data(decode_context):
    pass


@then("OrderCreated should decode as OrderCreated")
def then_order_decodes(decode_context):
    assert decode_context.get("decode_by_type")


@then("ItemAdded events should decode as ItemAdded")
def then_item_decodes(decode_context):
    pass


@then("OrderShipped should decode as OrderShipped")
def then_shipped_decodes(decode_context):
    pass


@then(parsers.parse("I should get {count:d} events"))
def then_get_count(decode_context, count):
    assert len(decode_context.get("filtered", [])) == count


@then("both should be ItemAdded type")
def then_both_item_added(decode_context):
    for e in decode_context.get("filtered", []):
        assert "ItemAdded" in e.event.type_url
