"""Tests for CommandHandlerGrpc."""

from dataclasses import dataclass
from unittest.mock import MagicMock

import grpc
import pytest
from google.protobuf import any_pb2

from angzarr_client import RejectionHandlerResponse
from angzarr_client.aggregate_handler import CommandHandlerGrpc
from angzarr_client.errors import CommandRejectedError
from angzarr_client.handler_protocols import CommandHandlerDomainHandler
from angzarr_client.proto.angzarr import types_pb2 as angzarr
from angzarr_client.router import CommandHandlerRouter
from angzarr_client.state_builder import StateRouter

# ============================================================================
# Test constants
# ============================================================================

DOMAIN_TEST = "test"
FULL_NAME_COMMAND_A = "test.CommandA"
TYPE_URL_COMMAND_A = "type.googleapis.com/test.CommandA"
TYPE_URL_UNKNOWN = "type.googleapis.com/test.UnknownCommand"


# ============================================================================
# Test state and handler
# ============================================================================


@dataclass
class FakeState:
    exists: bool = False


# Create a fake event type for StateRouter registration
class _FakeTestEvent:
    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeTestEvent"})()


def apply_fake_event(state: FakeState, event: _FakeTestEvent) -> None:
    """Apply any event to state (no-op for tests)."""
    pass


# StateRouter needs at least one event type registered
FAKE_STATE_ROUTER = StateRouter(FakeState).on(_FakeTestEvent, apply_fake_event)


class TestCommandHandlerImpl(CommandHandlerDomainHandler[FakeState]):
    """Test handler that emits events."""

    def __init__(
        self, handler_fn=None, should_reject=False, should_raise_value_error=False
    ):
        self._handler_fn = handler_fn
        self._should_reject = should_reject
        self._should_raise_value_error = should_raise_value_error

    def command_types(self) -> list[str]:
        return ["CommandA"]

    def state_router(self) -> StateRouter[FakeState]:
        return FAKE_STATE_ROUTER

    def handle(
        self,
        cmd_book: angzarr.CommandBook,
        payload: any_pb2.Any,
        state: FakeState,
        seq: int,
    ) -> angzarr.EventBook:
        if self._should_reject:
            raise CommandRejectedError("entity already exists")
        if self._should_raise_value_error:
            raise ValueError("name is required")
        if self._handler_fn:
            return self._handler_fn(cmd_book, payload, state, seq)
        return angzarr.EventBook(
            pages=[
                angzarr.EventPage(
                    event=any_pb2.Any(type_url=f"handled_a:seq={seq}", value=b""),
                ),
            ],
        )

    def on_rejected(
        self,
        notification: angzarr.Notification,
        state: FakeState,
        target_domain: str,
        target_command: str,
    ) -> RejectionHandlerResponse:
        return RejectionHandlerResponse()


# ============================================================================
# Helpers
# ============================================================================


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


# ============================================================================
# CommandHandlerGrpc tests
# ============================================================================


class TestCommandHandlerGrpcDispatch:
    def test_handle_dispatches_via_router(self):
        handler = TestCommandHandlerImpl()
        router = CommandHandlerRouter(DOMAIN_TEST, DOMAIN_TEST, handler)
        ch_handler = CommandHandlerGrpc(router)
        context = MagicMock(spec=grpc.ServicerContext)

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        resp = ch_handler.Handle(cmd, context)

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1
        assert resp.events.pages[0].event.type_url == "handled_a:seq=0"

    def test_handle_sync_dispatches_via_router(self):
        handler = TestCommandHandlerImpl()
        router = CommandHandlerRouter(DOMAIN_TEST, DOMAIN_TEST, handler)
        ch_handler = CommandHandlerGrpc(router)
        context = MagicMock(spec=grpc.ServicerContext)

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        resp = ch_handler.HandleSync(cmd, context)

        assert resp.WhichOneof("result") == "events"

    def test_maps_command_rejected_to_failed_precondition(self):
        handler = TestCommandHandlerImpl(should_reject=True)
        router = CommandHandlerRouter(DOMAIN_TEST, DOMAIN_TEST, handler)
        ch_handler = CommandHandlerGrpc(router)
        context = MagicMock(spec=grpc.ServicerContext)
        context.abort.side_effect = grpc.RpcError()

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        with pytest.raises(grpc.RpcError):
            ch_handler.Handle(cmd, context)

        context.abort.assert_called_once_with(
            grpc.StatusCode.FAILED_PRECONDITION,
            "entity already exists",
        )

    def test_maps_value_error_to_invalid_argument(self):
        handler = TestCommandHandlerImpl(should_raise_value_error=True)
        router = CommandHandlerRouter(DOMAIN_TEST, DOMAIN_TEST, handler)
        ch_handler = CommandHandlerGrpc(router)
        context = MagicMock(spec=grpc.ServicerContext)
        context.abort.side_effect = grpc.RpcError()

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        with pytest.raises(grpc.RpcError):
            ch_handler.Handle(cmd, context)

        context.abort.assert_called_once_with(
            grpc.StatusCode.INVALID_ARGUMENT,
            "name is required",
        )

    def test_unknown_command_maps_to_invalid_argument(self):
        handler = TestCommandHandlerImpl()
        router = CommandHandlerRouter(DOMAIN_TEST, DOMAIN_TEST, handler)
        ch_handler = CommandHandlerGrpc(router)
        context = MagicMock(spec=grpc.ServicerContext)
        context.abort.side_effect = grpc.RpcError()

        cmd = make_contextual_command(TYPE_URL_UNKNOWN)
        with pytest.raises(grpc.RpcError):
            ch_handler.Handle(cmd, context)

        context.abort.assert_called_once()
        assert context.abort.call_args[0][0] == grpc.StatusCode.INVALID_ARGUMENT

    def test_with_prior_events(self):
        prior = angzarr.EventBook(
            pages=[angzarr.EventPage(), angzarr.EventPage(), angzarr.EventPage()],
        )

        handler = TestCommandHandlerImpl()
        router = CommandHandlerRouter(DOMAIN_TEST, DOMAIN_TEST, handler)
        ch_handler = CommandHandlerGrpc(router)
        context = MagicMock(spec=grpc.ServicerContext)

        cmd = make_contextual_command(TYPE_URL_COMMAND_A, prior)
        resp = ch_handler.Handle(cmd, context)

        assert resp.events.pages[0].event.type_url == "handled_a:seq=3"


class TestCommandHandlerGrpcWithCommandHandlerClass:
    """Test CommandHandlerGrpc with CommandHandler class (OO approach)."""

    def test_command_handler_class_dispatch(self):
        from angzarr_client import CommandHandler, handles

        class FakeCommand:
            DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeCommand"})()

            def __init__(self, value: str = ""):
                self.value = value

            def SerializeToString(self, deterministic=None):
                return self.value.encode()

        class FakeEvent:
            DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeEvent"})()

            def __init__(self, result: str = ""):
                self.result = result

            def SerializeToString(self, deterministic=None):
                return self.result.encode()

        @dataclass
        class TestState:
            initialized: bool = False

        class TestCH(CommandHandler[TestState]):
            domain = "testch"

            def _create_empty_state(self):
                return TestState()

            def _apply_event(self, state, event_any):
                if "FakeEvent" in event_any.type_url:
                    state.initialized = True

            @handles(FakeCommand)
            def do_something(self, cmd: FakeCommand) -> FakeEvent:
                return FakeEvent(result="done")

        handler = CommandHandlerGrpc(TestCH)
        context = MagicMock(spec=grpc.ServicerContext)

        cmd = angzarr.ContextualCommand(
            command=angzarr.CommandBook(
                pages=[
                    angzarr.CommandPage(
                        command=any_pb2.Any(type_url="test.FakeCommand", value=b""),
                    ),
                ],
            ),
        )
        resp = handler.Handle(cmd, context)

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1

    def test_command_handler_class_domain_property(self):
        from angzarr_client import CommandHandler

        @dataclass
        class TestState:
            pass

        class TestCH(CommandHandler[TestState]):
            domain = "mych"

            def _create_empty_state(self):
                return TestState()

            def _apply_event(self, state, event_any):
                pass

        handler = CommandHandlerGrpc(TestCH)
        assert handler.domain == "mych"


class TestCommandHandlerGrpcReplay:
    """Test Replay RPC for CommandHandler class handlers."""

    def test_replay_with_command_handler_class(self):
        from google.protobuf import any_pb2

        from angzarr_client import CommandHandler, handles
        from angzarr_client.proto.angzarr import command_handler_pb2 as command_handler

        class FakeCommand:
            DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeCommand"})()

            def SerializeToString(self, deterministic=None):
                return b""

        class FakeEvent:
            DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeEvent"})()

            def __init__(self, result: str = ""):
                self.result = result

            def SerializeToString(self, deterministic=None):
                return self.result.encode()

        class TestState:
            """Protobuf-like state for Pack() compatibility."""

            DESCRIPTOR = type("Descriptor", (), {"full_name": "test.TestState"})()

            def __init__(self, initialized: bool = False):
                self.initialized = initialized

            def SerializeToString(self, deterministic=None):
                return b"1" if self.initialized else b"0"

        class ReplayableCH(CommandHandler[TestState]):
            domain = "replayable"

            def _create_empty_state(self):
                return TestState()

            def _apply_event(self, state, event_any):
                if "FakeEvent" in event_any.type_url:
                    state.initialized = True

            @handles(FakeCommand)
            def do_something(self, cmd: FakeCommand):
                return FakeEvent(result="done")

        handler = CommandHandlerGrpc(ReplayableCH)
        context = MagicMock(spec=grpc.ServicerContext)

        event_any = any_pb2.Any(type_url="test.FakeEvent", value=b"test")
        request = command_handler.ReplayRequest(
            events=[angzarr.EventPage(event=event_any)],
        )

        response = handler.Replay(request, context)

        # Should return computed state
        assert response.state.type_url != ""
        context.abort.assert_not_called()

    def test_replay_not_supported_for_command_handler_router(self):
        handler = TestCommandHandlerImpl()
        router = CommandHandlerRouter(DOMAIN_TEST, DOMAIN_TEST, handler)
        ch_handler = CommandHandlerGrpc(router)
        context = MagicMock(spec=grpc.ServicerContext)
        context.abort.side_effect = grpc.RpcError()

        from angzarr_client.proto.angzarr import command_handler_pb2 as command_handler

        request = command_handler.ReplayRequest()

        with pytest.raises(grpc.RpcError):
            ch_handler.Replay(request, context)

        context.abort.assert_called_once_with(
            grpc.StatusCode.UNIMPLEMENTED,
            "Replay not supported for router-based command handlers",
        )
