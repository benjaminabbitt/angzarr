"""Common step definitions shared across test features."""

from datetime import datetime, timezone

from behave import given, use_step_matcher
from google.protobuf.any_pb2 import Any as ProtoAny
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.proto.examples import table_pb2 as table
from angzarr_client.proto.examples import poker_types_pb2 as poker_types

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
