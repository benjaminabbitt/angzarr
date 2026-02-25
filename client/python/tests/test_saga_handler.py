"""Tests for SagaHandler."""

from unittest.mock import MagicMock

import grpc
from google.protobuf import any_pb2

from angzarr_client.handler_protocols import SagaDomainHandler
from angzarr_client.proto.angzarr import saga_pb2 as saga
from angzarr_client.proto.angzarr import types_pb2 as types
from angzarr_client.router import SagaRouter
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
# Test handler implementation
# ============================================================================


class TestSagaDomainHandler(SagaDomainHandler):
    """Test saga handler that emits commands."""

    def __init__(self, return_destinations=False):
        self._return_destinations = return_destinations

    def event_types(self) -> list[str]:
        return ["EventA"]

    def prepare(
        self,
        source: types.EventBook,
        event: any_pb2.Any,
    ) -> list[types.Cover]:
        if self._return_destinations:
            return [types.Cover(domain=DOMAIN_TARGET, root=source.cover.root)]
        return []

    def execute(
        self,
        source: types.EventBook,
        event: any_pb2.Any,
        destinations: list[types.EventBook],
    ) -> list[types.CommandBook]:
        return [
            types.CommandBook(
                cover=types.Cover(
                    domain=DOMAIN_TARGET,
                    root=source.cover.root,
                    correlation_id=source.cover.correlation_id,
                ),
            ),
        ]


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
                event=any_pb2.Any(type_url=type_url, value=b"\x01\x02\x03"),
            ),
        ],
    )


# ============================================================================
# Simple mode tests (router dispatch)
# ============================================================================


class TestSagaHandlerSimpleMode:
    def test_prepare_returns_empty_by_default(self):
        handler_impl = TestSagaDomainHandler()
        router = SagaRouter(SAGA_NAME, DOMAIN_SOURCE, handler_impl)
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_EVENT_A, CORR_ID_1, ROOT_BYTES)
        resp = handler.Prepare(saga.SagaPrepareRequest(source=book), context)

        assert len(resp.destinations) == 0

    def test_execute_dispatches_matching_event(self):
        handler_impl = TestSagaDomainHandler()
        router = SagaRouter(SAGA_NAME, DOMAIN_SOURCE, handler_impl)
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_EVENT_A, CORR_ID_1, ROOT_BYTES)
        resp = handler.Execute(saga.SagaExecuteRequest(source=book), context)

        assert len(resp.commands) == 1
        assert resp.commands[0].cover.domain == DOMAIN_TARGET
        assert resp.commands[0].cover.correlation_id == CORR_ID_1

    def test_execute_no_match_returns_empty(self):
        handler_impl = TestSagaDomainHandler()
        router = SagaRouter(SAGA_NAME, DOMAIN_SOURCE, handler_impl)
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_OTHER_EVENT, CORR_ID_1, ROOT_BYTES)
        resp = handler.Execute(saga.SagaExecuteRequest(source=book), context)

        # No matching event type, so empty result
        assert len(resp.commands) == 0


# ============================================================================
# Custom mode tests (with_prepare / with_execute)
# ============================================================================


class TestSagaHandlerCustomMode:
    def test_custom_prepare(self):
        handler_impl = TestSagaDomainHandler()
        router = SagaRouter(SAGA_NAME, DOMAIN_SOURCE, handler_impl)

        def custom_prepare(source):
            return [types.Cover(domain=DOMAIN_TARGET, root=source.cover.root)]

        handler = SagaHandler(router).with_prepare(custom_prepare)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_EVENT_A, CORR_ID_1, ROOT_BYTES)
        resp = handler.Prepare(saga.SagaPrepareRequest(source=book), context)

        assert len(resp.destinations) == 1
        assert resp.destinations[0].domain == DOMAIN_TARGET

    def test_custom_execute(self):
        handler_impl = TestSagaDomainHandler()
        router = SagaRouter(SAGA_NAME, DOMAIN_SOURCE, handler_impl)

        def custom_execute(source, destinations):
            return [
                types.CommandBook(
                    cover=types.Cover(
                        domain="custom", correlation_id=source.cover.correlation_id
                    ),
                ),
            ]

        handler = SagaHandler(router).with_execute(custom_execute)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_EVENT_A, CORR_ID_1, ROOT_BYTES)
        dest = types.EventBook(pages=[types.EventPage()])
        resp = handler.Execute(
            saga.SagaExecuteRequest(source=book, destinations=[dest]),
            context,
        )

        assert len(resp.commands) == 1
        assert resp.commands[0].cover.domain == "custom"
