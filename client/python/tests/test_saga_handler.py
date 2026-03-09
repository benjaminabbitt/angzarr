"""Tests for SagaHandler."""

from unittest.mock import MagicMock

import grpc
from google.protobuf import any_pb2

from angzarr_client.proto.angzarr import saga_pb2 as saga
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import SingleFluentRouter
from angzarr_client.saga_handler import SagaHandler

# ============================================================================
# Test constants
# ============================================================================

DOMAIN_SOURCE = "source"
DOMAIN_TARGET = "target"
FULL_NAME_EVENT_A = "test.EventA"
FULL_NAME_EVENT_B = "test.EventB"
TYPE_URL_EVENT_A = "type.googleapis.com/test.EventA"
TYPE_URL_OTHER_EVENT = "type.googleapis.com/test.OtherEvent"
CORR_ID_1 = "corr-1"
ROOT_BYTES = b"\x04\x05\x06"
SAGA_NAME = "test-saga"


# ============================================================================
# Test event fixture
# ============================================================================


class EventA:
    """Fake event for testing."""

    DESCRIPTOR = type("Descriptor", (), {"full_name": FULL_NAME_EVENT_A})()

    def __init__(self, value: str = ""):
        self.value = value

    def SerializeToString(self, deterministic=None):
        return self.value.encode()

    def ParseFromString(self, data: bytes):
        self.value = data.decode()


# ============================================================================
# Helpers
# ============================================================================


def make_event_book(type_url, correlation_id, root_bytes):
    return types.EventBook(
        cover=types.Cover(
            domain=DOMAIN_SOURCE,
            root=types.UUID(value=root_bytes),
            correlation_id=correlation_id,
        ),
        pages=[
            types.EventPage(
                header=types.PageHeader(sequence=0),
                payload=types.EventPage.Event(
                    event=any_pb2.Any(type_url=type_url, value=b"\x01\x02\x03"),
                ),
            ),
        ],
    )


def build_test_router() -> SingleFluentRouter:
    """Build a SingleFluentRouter for testing."""

    def execute_handler(event_any, root, correlation_id, destinations):
        return [
            types.CommandBook(
                cover=types.Cover(
                    domain=DOMAIN_TARGET,
                    root=root,
                    correlation_id=correlation_id,
                ),
            ),
        ]

    router = SingleFluentRouter(SAGA_NAME, DOMAIN_SOURCE)
    router.on(EventA, execute_handler)
    return router


# ============================================================================
# Simple mode tests (router dispatch)
# ============================================================================


class TestSagaHandlerSimpleMode:
    def test_handle_dispatches_matching_event(self):
        router = build_test_router()
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_EVENT_A, CORR_ID_1, ROOT_BYTES)
        resp = handler.Handle(saga.SagaHandleRequest(source=book), context)

        assert len(resp.commands) == 1
        assert resp.commands[0].cover.domain == DOMAIN_TARGET
        assert resp.commands[0].cover.correlation_id == CORR_ID_1

    def test_handle_no_match_returns_empty(self):
        router = build_test_router()
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_OTHER_EVENT, CORR_ID_1, ROOT_BYTES)
        resp = handler.Handle(saga.SagaHandleRequest(source=book), context)

        # No matching event type, so empty result
        assert len(resp.commands) == 0


# ============================================================================
# Custom mode tests (with_handle)
# ============================================================================


class TestSagaHandlerCustomMode:
    def test_custom_handle(self):
        router = build_test_router()

        def custom_handle(source):
            # Now returns SagaResponse (proto-generated type)
            return saga.SagaResponse(
                commands=[
                    types.CommandBook(
                        cover=types.Cover(
                            domain="custom", correlation_id=source.cover.correlation_id
                        ),
                    ),
                ]
            )

        handler = SagaHandler(router).with_handle(custom_handle)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_EVENT_A, CORR_ID_1, ROOT_BYTES)
        resp = handler.Handle(saga.SagaHandleRequest(source=book), context)

        assert len(resp.commands) == 1
        assert resp.commands[0].cover.domain == "custom"
