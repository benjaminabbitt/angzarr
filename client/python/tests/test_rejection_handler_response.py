"""Integration tests for RejectionHandlerResponse.

Tests the unified response type for rejection handlers that can return
both compensation events AND upstream notification.
"""

from google.protobuf.any_pb2 import Any as ProtoAny

from angzarr_client.compensation import RejectionHandlerResponse
from angzarr_client.proto.angzarr import types_pb2 as types


# =============================================================================
# Test Fixtures
# =============================================================================


def make_notification(
    domain: str = "test",
    command_type: str = "TestCommand",
    reason: str = "test_rejection",
) -> types.Notification:
    """Create a test Notification with RejectionNotification payload."""
    cmd_any = ProtoAny(
        type_url=f"type.googleapis.com/test.{command_type}",
        value=b"",
    )
    rejected_cmd = types.CommandBook(
        cover=types.Cover(domain=domain),
        pages=[types.CommandPage(command=cmd_any)],
    )
    rejection = types.RejectionNotification(
        issuer_name="test-saga",
        issuer_type="saga",
        rejection_reason=reason,
        rejected_command=rejected_cmd,
    )
    payload = ProtoAny()
    payload.Pack(rejection, type_url_prefix="type.googleapis.com/")
    return types.Notification(payload=payload)


def make_event_book(domain: str = "test") -> types.EventBook:
    """Create a test EventBook."""
    return types.EventBook(
        cover=types.Cover(domain=domain),
        pages=[types.EventPage(event=ProtoAny(type_url="type.googleapis.com/test.TestEvent"))],
    )


# =============================================================================
# RejectionHandlerResponse Tests
# =============================================================================


class TestRejectionHandlerResponse:
    """Tests for the RejectionHandlerResponse dataclass."""

    def test_empty_response(self):
        """Empty response has no events or notification."""
        response = RejectionHandlerResponse()
        assert response.events is None
        assert response.notification is None

    def test_events_only(self):
        """Response can contain only events."""
        event_book = make_event_book()
        response = RejectionHandlerResponse(events=event_book)
        assert response.events is event_book
        assert response.notification is None

    def test_notification_only(self):
        """Response can contain only notification."""
        notification = make_notification()
        response = RejectionHandlerResponse(notification=notification)
        assert response.events is None
        assert response.notification is notification

    def test_both_events_and_notification(self):
        """Response can contain both events and notification."""
        event_book = make_event_book()
        notification = make_notification()
        response = RejectionHandlerResponse(events=event_book, notification=notification)
        assert response.events is event_book
        assert response.notification is notification


class TestRejectionHandlerResponseMultipleEvents:
    """Additional tests for RejectionHandlerResponse with multiple events."""

    def test_events_pages_accessible(self):
        """Response events pages are accessible and countable."""
        event_book = types.EventBook(
            pages=[
                types.EventPage(event=ProtoAny(type_url="type.googleapis.com/test.Event1")),
                types.EventPage(event=ProtoAny(type_url="type.googleapis.com/test.Event2")),
            ]
        )
        response = RejectionHandlerResponse(events=event_book)

        assert response.events is not None
        assert len(response.events.pages) == 2

    def test_notification_payload_accessible(self):
        """Response notification payload is accessible."""
        notification = make_notification(domain="test", command_type="TestCmd")
        response = RejectionHandlerResponse(notification=notification)

        assert response.notification is not None
        assert response.notification.HasField("payload")
