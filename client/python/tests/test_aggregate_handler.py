"""Tests for AggregateHandler."""

from unittest.mock import MagicMock

import grpc
import pytest
from google.protobuf import any_pb2

from angzarr_client.proto.angzarr import types_pb2 as angzarr
from angzarr_client.aggregate_handler import AggregateHandler
from angzarr_client.errors import CommandRejectedError
from angzarr_client.router import COMPONENT_AGGREGATE, CommandRouter

# ============================================================================
# Test constants
# ============================================================================

DOMAIN_TEST = "test"
SUFFIX_COMMAND_A = "CommandA"
TYPE_URL_COMMAND_A = "type.test/CommandA"
TYPE_URL_UNKNOWN = "type.test/UnknownCommand"


# ============================================================================
# Helpers
# ============================================================================


class FakeState:
    def __init__(self, exists: bool = False):
        self.exists = exists


def dummy_rebuild(events):
    return FakeState()


def make_contextual_command(type_url, prior_events=None):
    cmd = angzarr.ContextualCommand(
        command=angzarr.CommandBook(
            cover=angzarr.Cover(domain=DOMAIN_TEST),
            pages=[
                angzarr.CommandPage(
                    command=any_pb2.Any(type_url=type_url, value=b""),
                ),
            ],
        ),
    )
    if prior_events is not None:
        cmd.events.CopyFrom(prior_events)
    return cmd


def handler_a(command_book, command_any, state, seq):
    return angzarr.EventBook(
        pages=[
            angzarr.EventPage(
                event=any_pb2.Any(type_url=f"handled_a:seq={seq}", value=b""),
            ),
        ],
    )


def rejecting_handler(command_book, command_any, state, seq):
    raise CommandRejectedError("entity already exists")


def invalid_handler(command_book, command_any, state, seq):
    raise ValueError("name is required")


# ============================================================================
# AggregateHandler tests
# ============================================================================


class TestAggregateHandlerDispatch:
    def test_handle_dispatches_via_router(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, handler_a
        )
        handler = AggregateHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        resp = handler.Handle(cmd, context)

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1
        assert resp.events.pages[0].event.type_url == "handled_a:seq=0"

    def test_handle_sync_dispatches_via_router(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, handler_a
        )
        handler = AggregateHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        resp = handler.HandleSync(cmd, context)

        assert resp.WhichOneof("result") == "events"

    def test_maps_command_rejected_to_failed_precondition(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, rejecting_handler
        )
        handler = AggregateHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)
        context.abort.side_effect = grpc.RpcError()

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        with pytest.raises(grpc.RpcError):
            handler.Handle(cmd, context)

        context.abort.assert_called_once_with(
            grpc.StatusCode.FAILED_PRECONDITION,
            "entity already exists",
        )

    def test_maps_value_error_to_invalid_argument(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, invalid_handler
        )
        handler = AggregateHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)
        context.abort.side_effect = grpc.RpcError()

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        with pytest.raises(grpc.RpcError):
            handler.Handle(cmd, context)

        context.abort.assert_called_once_with(
            grpc.StatusCode.INVALID_ARGUMENT,
            "name is required",
        )

    def test_unknown_command_maps_to_invalid_argument(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, handler_a
        )
        handler = AggregateHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)
        context.abort.side_effect = grpc.RpcError()

        cmd = make_contextual_command(TYPE_URL_UNKNOWN)
        with pytest.raises(grpc.RpcError):
            handler.Handle(cmd, context)

        context.abort.assert_called_once()
        assert context.abort.call_args[0][0] == grpc.StatusCode.INVALID_ARGUMENT

    def test_with_prior_events(self):
        prior = angzarr.EventBook(
            pages=[angzarr.EventPage(), angzarr.EventPage(), angzarr.EventPage()],
        )

        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, handler_a
        )
        handler = AggregateHandler(router)
        context = MagicMock(spec=grpc.ServicerContext)

        cmd = make_contextual_command(TYPE_URL_COMMAND_A, prior)
        resp = handler.Handle(cmd, context)

        assert resp.events.pages[0].event.type_url == "handled_a:seq=3"


class TestAggregateHandlerDescriptor:
    def test_descriptor(self):
        router = (
            CommandRouter("order", dummy_rebuild)
            .on("CreateOrder", handler_a)
            .on("CancelOrder", handler_a)
        )
        handler = AggregateHandler(router)
        desc = handler.descriptor()

        assert desc.name == "order"
        assert desc.component_type == COMPONENT_AGGREGATE
        assert len(desc.inputs) == 1
        assert len(desc.inputs[0].types) == 2
