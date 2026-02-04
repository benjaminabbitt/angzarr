"""Tests for SagaHandler."""

from unittest.mock import MagicMock

import grpc
from google.protobuf import any_pb2

from angzarr import types_pb2 as types
from router import COMPONENT_SAGA, EventRouter
from saga_handler import SagaHandler

# ============================================================================
# Test constants
# ============================================================================

DOMAIN_ORDER = "order"
DOMAIN_FULFILLMENT = "fulfillment"
SUFFIX_ORDER_COMPLETED = "OrderCompleted"
SUFFIX_ORDER_CANCELLED = "OrderCancelled"
TYPE_URL_ORDER_DONE = "type.examples/examples.OrderCompleted"
TYPE_URL_OTHER_EVENT = "type.examples/examples.SomethingElse"
CORR_ID_1 = "corr-1"
ROOT_BYTES = b"\x04\x05\x06"
SAGA_FULFILLMENT = "fulfillment"


# ============================================================================
# Helpers
# ============================================================================


def saga_event_handler(event, root, correlation_id):
    return [
        types.CommandBook(
            cover=types.Cover(domain=DOMAIN_FULFILLMENT, root=root, correlation_id=correlation_id),
        ),
    ]


def make_event_book(type_url, correlation_id, root_bytes):
    return types.EventBook(
        cover=types.Cover(
            domain=DOMAIN_ORDER,
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
        router = (
            EventRouter(SAGA_FULFILLMENT, DOMAIN_ORDER)
            .output(DOMAIN_FULFILLMENT)
            .on(SUFFIX_ORDER_COMPLETED, saga_event_handler)
        )
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_ORDER_DONE, CORR_ID_1, ROOT_BYTES)
        resp = handler.Prepare(types.SagaPrepareRequest(source=book), context)

        assert len(resp.destinations) == 0

    def test_execute_dispatches_matching_event(self):
        router = (
            EventRouter(SAGA_FULFILLMENT, DOMAIN_ORDER)
            .output(DOMAIN_FULFILLMENT)
            .on(SUFFIX_ORDER_COMPLETED, saga_event_handler)
        )
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_ORDER_DONE, CORR_ID_1, ROOT_BYTES)
        resp = handler.Execute(types.SagaExecuteRequest(source=book), context)

        assert len(resp.commands) == 1
        assert resp.commands[0].cover.domain == DOMAIN_FULFILLMENT
        assert resp.commands[0].cover.correlation_id == CORR_ID_1

    def test_execute_no_match_returns_empty(self):
        router = (
            EventRouter(SAGA_FULFILLMENT, DOMAIN_ORDER)
            .on(SUFFIX_ORDER_COMPLETED, saga_event_handler)
        )
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_OTHER_EVENT, CORR_ID_1, ROOT_BYTES)
        resp = handler.Execute(types.SagaExecuteRequest(source=book), context)

        assert len(resp.commands) == 0


# ============================================================================
# Custom mode tests (with_prepare / with_execute)
# ============================================================================


class TestSagaHandlerCustomMode:
    def test_custom_prepare(self):
        router = (
            EventRouter(SAGA_FULFILLMENT, DOMAIN_ORDER)
            .on(SUFFIX_ORDER_COMPLETED, saga_event_handler)
        )

        def custom_prepare(source):
            return [types.Cover(domain=DOMAIN_FULFILLMENT, root=source.cover.root)]

        handler = SagaHandler(router).with_prepare(custom_prepare)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_ORDER_DONE, CORR_ID_1, ROOT_BYTES)
        resp = handler.Prepare(types.SagaPrepareRequest(source=book), context)

        assert len(resp.destinations) == 1
        assert resp.destinations[0].domain == DOMAIN_FULFILLMENT

    def test_custom_execute(self):
        router = (
            EventRouter(SAGA_FULFILLMENT, DOMAIN_ORDER)
            .on(SUFFIX_ORDER_COMPLETED, saga_event_handler)
        )

        def custom_execute(source, destinations):
            return [
                types.CommandBook(
                    cover=types.Cover(domain="custom", correlation_id=source.cover.correlation_id),
                ),
            ]

        handler = SagaHandler(router).with_execute(custom_execute)
        context = MagicMock(spec=grpc.ServicerContext)

        book = make_event_book(TYPE_URL_ORDER_DONE, CORR_ID_1, ROOT_BYTES)
        dest = types.EventBook(pages=[types.EventPage()])
        resp = handler.Execute(
            types.SagaExecuteRequest(source=book, destinations=[dest]),
            context,
        )

        assert len(resp.commands) == 1
        assert resp.commands[0].cover.domain == "custom"


# ============================================================================
# Descriptor tests
# ============================================================================


class TestSagaHandlerDescriptor:
    def test_descriptor(self):
        router = (
            EventRouter(SAGA_FULFILLMENT, DOMAIN_ORDER)
            .output(DOMAIN_FULFILLMENT)
            .on(SUFFIX_ORDER_COMPLETED, saga_event_handler)
            .on(SUFFIX_ORDER_CANCELLED, saga_event_handler)
        )
        handler = SagaHandler(router)
        desc = handler.descriptor()

        assert desc.name == SAGA_FULFILLMENT
        assert desc.component_type == COMPONENT_SAGA
        assert len(desc.inputs) == 1
        assert len(desc.inputs[0].event_types) == 2

    def test_grpc_get_descriptor(self):
        router = (
            EventRouter(SAGA_FULFILLMENT, DOMAIN_ORDER)
            .output(DOMAIN_FULFILLMENT)
            .on(SUFFIX_ORDER_COMPLETED, saga_event_handler)
        )
        handler = SagaHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        resp = handler.GetDescriptor(types.GetDescriptorRequest(), context)

        assert resp.name == SAGA_FULFILLMENT
        assert resp.component_type == COMPONENT_SAGA
        assert len(resp.inputs) == 1
        assert resp.inputs[0].domain == DOMAIN_ORDER
