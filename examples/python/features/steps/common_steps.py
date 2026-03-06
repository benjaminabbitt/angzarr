"""Common step definitions shared across test features."""

from datetime import datetime, timezone

from behave import given, then, use_step_matcher
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.helpers import type_name_from_url
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import poker_types_pb2 as poker_types
from angzarr_client.proto.examples import table_pb2 as table

# Use regex matchers for flexibility
use_step_matcher("re")


def make_timestamp():
    """Create current timestamp."""
    return Timestamp(seconds=int(datetime.now(timezone.utc).timestamp()))


def make_event_page(event_msg, num: int = 0, time_str: str = None) -> types.EventPage:
    """Create EventPage with packed event."""
    event_any = ProtoAny()
    event_any.Pack(event_msg, type_url_prefix="type.googleapis.com/")

    created_at = None
    if time_str:
        h, m, s = map(int, time_str.split(":"))
        dt = datetime(2024, 1, 1, h, m, s, tzinfo=timezone.utc)
        created_at = Timestamp(seconds=int(dt.timestamp()))
    else:
        created_at = make_timestamp()

    return types.EventPage(
        num=num,
        event=event_any,
        created_at=created_at,
    )


# --- Then steps for result event assertions ---
# These handle the `examples.EventType` format used in feature files


@then(r"the result is an? examples\.(?P<event_type>\w+) event")
def step_then_result_is_examples_event(context, event_type):
    """Verify the result event type (handles examples.EventType format).

    Matches patterns like:
    - Then the result is a examples.CardsDealt event
    - Then the result is an examples.ActionTaken event
    """
    assert (
        context.result is not None
    ), f"Expected {event_type} event but got error: {getattr(context, 'error_message', context.error)}"
    assert context.result.pages, "No event pages in result"
    event_any = context.result.pages[0].event
    actual_type = type_name_from_url(event_any.type_url)
    assert actual_type == event_type, f"Expected {event_type} but got {actual_type}"
