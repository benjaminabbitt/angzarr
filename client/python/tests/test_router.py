"""Tests for CommandRouter and EventRouter."""

import pytest
from google.protobuf import any_pb2

from angzarr_client.proto.angzarr import types_pb2 as angzarr
from angzarr_client.router import (
    ERRMSG_NO_COMMAND_PAGES,
    ERRMSG_UNKNOWN_COMMAND,
    CommandRouter,
    EventRouter,
    next_sequence,
    command_handler,
    _pack_events,
    _pack_any,
)

# ============================================================================
# Test constants — reused across test cases
# ============================================================================

DOMAIN_TEST = "test"
DOMAIN_ORDER = "order"
DOMAIN_FULFILLMENT = "fulfillment"
DOMAIN_INVENTORY = "inventory"
DOMAIN_CART = "cart"

SUFFIX_COMMAND_A = "CommandA"
SUFFIX_COMMAND_B = "CommandB"
SUFFIX_CREATE = "Create"
SUFFIX_CREATE_ORDER = "CreateOrder"
SUFFIX_CANCEL_ORDER = "CancelOrder"
SUFFIX_ORDER_COMPLETED = "OrderCompleted"
SUFFIX_ORDER_CANCELLED = "OrderCancelled"
SUFFIX_QTY_UPDATED = "QuantityUpdated"

TYPE_URL_COMMAND_A = "type.test/CommandA"
TYPE_URL_COMMAND_B = "type.test/CommandB"
TYPE_URL_UNKNOWN = "type.test/UnknownCommand"
TYPE_URL_CREATE = "type.test/Create"
TYPE_URL_FULL_CREATE = "type.examples/examples.CreateOrder"
TYPE_URL_ORDER_DONE = "type.examples/examples.OrderCompleted"
TYPE_URL_OTHER_EVENT = "type.examples/examples.SomethingElse"
TYPE_URL_QTY_UPDATED = "type.examples/examples.QuantityUpdated"

CORR_ID_1 = "corr-1"
CORR_ID_2 = "corr-2"

SAGA_FULFILLMENT = "fulfillment"
SAGA_TEST = "test-saga"
SAGA_INVENTORY_RESERVE = "inventory-reservation"

ROOT_BYTES_A = b"\x04\x05\x06"
ROOT_BYTES_B = b"\x01\x02"


# ============================================================================
# Helpers
# ============================================================================


class FakeState:
    def __init__(self, exists: bool = False):
        self.exists = exists


def dummy_rebuild(events):
    return FakeState()


def exists_rebuild(events):
    return FakeState(exists=True)


def make_contextual_command(type_url, prior_events=None):
    """Create a ContextualCommand for testing."""
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
                event=any_pb2.Any(
                    type_url=f"handled_a:seq={seq}",
                    value=b"",
                ),
            ),
        ],
    )


def handler_b(command_book, command_any, state, seq):
    return angzarr.EventBook()


# ============================================================================
# CommandRouter tests
# ============================================================================


class TestCommandRouterDispatch:
    def test_dispatches_correct_handler(self):
        router = (
            CommandRouter(DOMAIN_TEST, dummy_rebuild)
            .on(SUFFIX_COMMAND_A, handler_a)
            .on(SUFFIX_COMMAND_B, handler_b)
        )

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        resp = router.dispatch(cmd)

        assert resp.WhichOneof("result") == "events"
        events = resp.events
        assert len(events.pages) == 1
        assert events.pages[0].event.type_url == "handled_a:seq=0"

    def test_dispatches_second_handler(self):
        router = (
            CommandRouter(DOMAIN_TEST, dummy_rebuild)
            .on(SUFFIX_COMMAND_A, handler_a)
            .on(SUFFIX_COMMAND_B, handler_b)
        )

        cmd = make_contextual_command(TYPE_URL_COMMAND_B)
        resp = router.dispatch(cmd)

        assert resp.WhichOneof("result") == "events"

    def test_rebuild_receives_prior_events(self):
        prior = angzarr.EventBook(
            pages=[
                angzarr.EventPage(event=any_pb2.Any(type_url="e1")),
                angzarr.EventPage(event=any_pb2.Any(type_url="e2")),
                angzarr.EventPage(event=any_pb2.Any(type_url="e3")),
            ],
        )

        cmd = make_contextual_command(TYPE_URL_COMMAND_A, prior)
        resp = (
            CommandRouter(DOMAIN_TEST, dummy_rebuild)
            .on(SUFFIX_COMMAND_A, handler_a)
            .dispatch(cmd)
        )

        assert resp.events.pages[0].event.type_url == "handled_a:seq=3"

    def test_unknown_command_raises(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, handler_a
        )

        cmd = make_contextual_command(TYPE_URL_UNKNOWN)
        with pytest.raises(ValueError, match=ERRMSG_UNKNOWN_COMMAND):
            router.dispatch(cmd)

    def test_handler_error_propagates(self):
        def reject_handler(command_book, command_any, state, seq):
            if state.exists:
                raise RuntimeError("already exists")
            return angzarr.EventBook()

        router = CommandRouter(DOMAIN_TEST, exists_rebuild).on(
            SUFFIX_CREATE, reject_handler
        )

        cmd = make_contextual_command(TYPE_URL_CREATE)
        with pytest.raises(RuntimeError, match="already exists"):
            router.dispatch(cmd)

    def test_no_command_pages_raises(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, handler_a
        )

        cmd = angzarr.ContextualCommand(
            command=angzarr.CommandBook(cover=angzarr.Cover(domain=DOMAIN_TEST)),
        )
        with pytest.raises(ValueError, match=ERRMSG_NO_COMMAND_PAGES):
            router.dispatch(cmd)

    def test_suffix_matching_with_full_type_url(self):
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_CREATE_ORDER, handler_a
        )

        cmd = make_contextual_command(TYPE_URL_FULL_CREATE)
        resp = router.dispatch(cmd)
        assert resp is not None

    def test_passes_args_to_handler(self):
        captured = {}

        def capturing_handler(command_book, command_any, state, seq):
            captured["command_book"] = command_book
            captured["command_any"] = command_any
            captured["state"] = state
            captured["seq"] = seq
            return angzarr.EventBook()

        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            SUFFIX_COMMAND_A, capturing_handler
        )

        cmd = make_contextual_command(TYPE_URL_COMMAND_A)
        router.dispatch(cmd)

        assert isinstance(captured["command_book"], angzarr.CommandBook)
        assert isinstance(captured["command_any"], any_pb2.Any)
        assert captured["command_any"].type_url == TYPE_URL_COMMAND_A
        assert isinstance(captured["state"], FakeState)
        assert captured["seq"] == 0


class TestCommandRouterMetadata:
    def test_domain(self):
        router = CommandRouter(DOMAIN_ORDER, dummy_rebuild)
        assert router.domain == DOMAIN_ORDER


# ============================================================================
# next_sequence tests
# ============================================================================


class TestNextSequence:
    def test_none_events(self):
        assert next_sequence(None) == 0

    def test_empty_pages(self):
        assert next_sequence(angzarr.EventBook()) == 0

    def test_with_pages(self):
        events = angzarr.EventBook(
            pages=[
                angzarr.EventPage(),
                angzarr.EventPage(),
                angzarr.EventPage(),
            ],
        )
        assert next_sequence(events) == 3


# ============================================================================
# EventRouter tests
# ============================================================================


def saga_handler(event, root, correlation_id, destinations):
    return [
        angzarr.CommandBook(
            cover=angzarr.Cover(
                domain=DOMAIN_FULFILLMENT, root=root, correlation_id=correlation_id
            ),
        ),
    ]


def multi_command_handler(event, root, correlation_id, destinations):
    return [
        angzarr.CommandBook(
            cover=angzarr.Cover(
                domain=DOMAIN_INVENTORY, root=root, correlation_id=correlation_id
            ),
        ),
        angzarr.CommandBook(
            cover=angzarr.Cover(
                domain=DOMAIN_INVENTORY, root=root, correlation_id=correlation_id
            ),
        ),
    ]


def make_event_book(type_url, correlation_id, root_bytes, domain=DOMAIN_ORDER):
    """Create an EventBook for testing."""
    return angzarr.EventBook(
        cover=angzarr.Cover(
            domain=domain,
            root=angzarr.UUID(value=root_bytes),
            correlation_id=correlation_id,
        ),
        pages=[
            angzarr.EventPage(
                event=any_pb2.Any(type_url=type_url, value=b"\x01\x02\x03"),
            ),
        ],
    )


class TestEventRouterDispatch:
    def test_dispatches_matching_event(self):
        router = (
            EventRouter(SAGA_TEST)
            .domain(DOMAIN_ORDER)
            .on(SUFFIX_ORDER_COMPLETED, saga_handler)
        )

        book = make_event_book(TYPE_URL_ORDER_DONE, CORR_ID_1, ROOT_BYTES_A)
        commands = router.dispatch(book)

        assert len(commands) == 1
        assert commands[0].cover.domain == DOMAIN_FULFILLMENT
        assert commands[0].cover.correlation_id == CORR_ID_1
        assert commands[0].cover.root.value == ROOT_BYTES_A

    def test_skips_unmatched_event(self):
        router = (
            EventRouter(SAGA_TEST)
            .domain(DOMAIN_ORDER)
            .on(SUFFIX_ORDER_COMPLETED, saga_handler)
        )

        book = make_event_book(TYPE_URL_OTHER_EVENT, CORR_ID_1, ROOT_BYTES_A)
        commands = router.dispatch(book)

        assert commands == []

    def test_multiple_commands(self):
        router = (
            EventRouter(SAGA_INVENTORY_RESERVE)
            .domain(DOMAIN_CART)
            .on(SUFFIX_QTY_UPDATED, multi_command_handler)
        )

        book = make_event_book(
            TYPE_URL_QTY_UPDATED, CORR_ID_2, ROOT_BYTES_B, DOMAIN_CART
        )
        commands = router.dispatch(book)

        assert len(commands) == 2

    def test_multiple_pages(self):
        router = (
            EventRouter(SAGA_TEST)
            .domain(DOMAIN_ORDER)
            .on(SUFFIX_ORDER_COMPLETED, saga_handler)
        )

        book = angzarr.EventBook(
            cover=angzarr.Cover(
                domain=DOMAIN_ORDER,
                root=angzarr.UUID(value=b"\x01"),
                correlation_id=CORR_ID_1,
            ),
            pages=[
                angzarr.EventPage(event=any_pb2.Any(type_url=TYPE_URL_OTHER_EVENT)),
                angzarr.EventPage(event=any_pb2.Any(type_url=TYPE_URL_ORDER_DONE)),
            ],
        )
        commands = router.dispatch(book)

        assert len(commands) == 1


class TestEventRouterMetadata:
    def test_name(self):
        router = EventRouter(SAGA_FULFILLMENT).domain(DOMAIN_ORDER)
        assert router.name == SAGA_FULFILLMENT

    def test_input_domain(self):
        router = EventRouter(SAGA_FULFILLMENT).domain(DOMAIN_ORDER)
        assert router.input_domain == DOMAIN_ORDER


# ============================================================================
# @command_handler decorator tests
# ============================================================================


class FakeCommand:
    """Fake protobuf command for testing."""

    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeCommand"})()

    def __init__(self, value: str = ""):
        self.value = value

    def SerializeToString(self, deterministic=None):
        return self.value.encode()

    def ParseFromString(self, data: bytes):
        self.value = data.decode()


class FakeEvent:
    """Fake protobuf event for testing."""

    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeEvent"})()

    def __init__(self, result: str = ""):
        self.result = result

    def SerializeToString(self, deterministic=None):
        return self.result.encode()

    def ParseFromString(self, data: bytes):
        self.result = data.decode()


class AnotherCommand:
    """Another fake command for testing."""

    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.AnotherCommand"})()

    def __init__(self, name: str = ""):
        self.name = name

    def SerializeToString(self, deterministic=None):
        return self.name.encode()

    def ParseFromString(self, data: bytes):
        self.name = data.decode()


class TestCommandHandlerDecorator:
    def test_decorator_validates_missing_param(self):
        with pytest.raises(TypeError, match="must have cmd parameter"):

            @command_handler(FakeCommand)
            def bad_handler():
                pass

    def test_decorator_validates_missing_type_hint(self):
        with pytest.raises(TypeError, match="missing type hint"):

            @command_handler(FakeCommand)
            def bad_handler(cmd):
                pass

    def test_decorator_validates_type_hint_mismatch(self):
        with pytest.raises(TypeError, match="doesn't match type hint"):

            @command_handler(FakeCommand)
            def bad_handler(cmd: AnotherCommand):
                pass

    def test_decorator_preserves_function_name(self):
        @command_handler(FakeCommand)
        def handle_create(cmd: FakeCommand, state, seq: int):
            return FakeEvent()

        assert handle_create.__name__ == "handle_create"

    def test_decorator_stores_command_type(self):
        @command_handler(FakeCommand)
        def handle_create(cmd: FakeCommand, state, seq: int):
            return FakeEvent()

        assert handle_create._command_type == FakeCommand

    def test_decorated_handler_unpacks_command(self):
        captured = {}

        @command_handler(FakeCommand)
        def handle_create(cmd: FakeCommand, state, seq: int):
            captured["cmd"] = cmd
            captured["state"] = state
            captured["seq"] = seq
            return FakeEvent(result="done")

        # Create command_any with FakeCommand packed
        cmd = FakeCommand(value="test_value")
        command_any = any_pb2.Any()
        command_any.Pack(cmd)

        command_book = angzarr.CommandBook()
        result = handle_create(command_book, command_any, FakeState(), 5)

        assert isinstance(captured["cmd"], FakeCommand)
        assert captured["cmd"].value == "test_value"
        assert isinstance(captured["state"], FakeState)
        assert captured["seq"] == 5
        assert isinstance(result, angzarr.EventBook)

    def test_decorated_handler_packs_single_event(self):
        @command_handler(FakeCommand)
        def handle_create(cmd: FakeCommand, state, seq: int):
            return FakeEvent(result="created")

        cmd = FakeCommand()
        command_any = any_pb2.Any()
        command_any.Pack(cmd)

        result = handle_create(angzarr.CommandBook(), command_any, FakeState(), 0)

        assert isinstance(result, angzarr.EventBook)
        assert len(result.pages) == 1
        assert "FakeEvent" in result.pages[0].event.type_url

    def test_decorated_handler_packs_multiple_events(self):
        @command_handler(FakeCommand)
        def handle_multi(cmd: FakeCommand, state, seq: int):
            return (FakeEvent(result="first"), FakeEvent(result="second"))

        cmd = FakeCommand()
        command_any = any_pb2.Any()
        command_any.Pack(cmd)

        result = handle_multi(angzarr.CommandBook(), command_any, FakeState(), 0)

        assert isinstance(result, angzarr.EventBook)
        assert len(result.pages) == 2

    def test_decorated_handler_handles_none_return(self):
        @command_handler(FakeCommand)
        def handle_noop(cmd: FakeCommand, state, seq: int):
            return None

        cmd = FakeCommand()
        command_any = any_pb2.Any()
        command_any.Pack(cmd)

        result = handle_noop(angzarr.CommandBook(), command_any, FakeState(), 0)

        assert isinstance(result, angzarr.EventBook)
        assert len(result.pages) == 0

    def test_integration_with_command_router(self):
        @command_handler(FakeCommand)
        def handle_create(cmd: FakeCommand, state, seq: int):
            return FakeEvent(result=f"created:{cmd.value}")

        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(
            "FakeCommand", handle_create
        )

        # Create contextual command
        cmd = FakeCommand(value="test123")
        cmd_any = any_pb2.Any()
        cmd_any.Pack(cmd)

        ctx_cmd = angzarr.ContextualCommand(
            command=angzarr.CommandBook(
                pages=[angzarr.CommandPage(command=cmd_any)],
            ),
        )

        resp = router.dispatch(ctx_cmd)

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1

    def test_integration_with_reflection(self):
        """Test single-argument .on() that derives suffix from decorator."""

        @command_handler(FakeCommand)
        def handle_create(cmd: FakeCommand, state, seq: int):
            return FakeEvent(result=f"reflected:{cmd.value}")

        # Single argument - suffix derived from @command_handler
        router = CommandRouter(DOMAIN_TEST, dummy_rebuild).on(handle_create)

        # Create contextual command
        cmd = FakeCommand(value="test456")
        cmd_any = any_pb2.Any()
        cmd_any.Pack(cmd)

        ctx_cmd = angzarr.ContextualCommand(
            command=angzarr.CommandBook(
                pages=[angzarr.CommandPage(command=cmd_any)],
            ),
        )

        resp = router.dispatch(ctx_cmd)

        assert resp.WhichOneof("result") == "events"
        assert len(resp.events.pages) == 1

    def test_reflection_requires_decorator(self):
        """Test that single-argument .on() requires @command_handler."""

        def plain_handler(cb, cmd_any, state, seq):
            return angzarr.EventBook()

        with pytest.raises(TypeError, match="must be decorated with @command_handler"):
            CommandRouter(DOMAIN_TEST, dummy_rebuild).on(plain_handler)


class TestPackEvents:
    def test_pack_single_event(self):
        event = FakeEvent(result="test")
        result = _pack_events(event)

        assert isinstance(result, angzarr.EventBook)
        assert len(result.pages) == 1

    def test_pack_tuple_of_events(self):
        events = (FakeEvent(result="a"), FakeEvent(result="b"))
        result = _pack_events(events)

        assert len(result.pages) == 2

    def test_pack_none(self):
        result = _pack_events(None)

        assert isinstance(result, angzarr.EventBook)
        assert len(result.pages) == 0


class TestPackAny:
    def test_pack_any_sets_type_url(self):
        event = FakeEvent(result="test")
        result = _pack_any(event)

        assert isinstance(result, any_pb2.Any)
        assert "FakeEvent" in result.type_url
