"""Tests for Aggregate ABC and @handles decorator."""

import pytest
from google.protobuf import any_pb2

from angzarr_client import Aggregate, handles
from angzarr_client.errors import CommandRejectedError
from angzarr_client.proto.angzarr import types_pb2 as types


# =============================================================================
# Test fixtures - minimal protobuf-like messages for testing
# =============================================================================


class FakeCommand:
    """Fake command message for testing."""
    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeCommand"})()

    def __init__(self, value: str = ""):
        self.value = value

    def SerializeToString(self, deterministic=None):
        return self.value.encode()


class AnotherCommand:
    """Another fake command for testing."""
    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.AnotherCommand"})()

    def __init__(self, name: str = ""):
        self.name = name

    def SerializeToString(self, deterministic=None):
        return self.name.encode()


class FakeEvent:
    """Fake event message for testing."""
    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.FakeEvent"})()

    def __init__(self, result: str = ""):
        self.result = result

    def SerializeToString(self, deterministic=None):
        return self.result.encode()


class AggState:
    """State for test aggregate - protobuf-like for Pack() compatibility."""

    DESCRIPTOR = type("Descriptor", (), {"full_name": "test.AggState"})()

    def __init__(self, initialized: bool = False, value: str = ""):
        self.initialized = initialized
        self.value = value

    def SerializeToString(self, deterministic=None):
        # Simple serialization for testing
        return f"{self.initialized}:{self.value}".encode()


# =============================================================================
# Test aggregates
# =============================================================================


class SampleAggregate(Aggregate[AggState]):
    """Test aggregate for unit tests."""

    domain = "test"

    def _create_empty_state(self) -> AggState:
        return AggState()

    def _apply_event(self, state: AggState, event_any) -> None:
        if "FakeEvent" in event_any.type_url:
            state.initialized = True
            state.value = event_any.value.decode() if event_any.value else ""

    @handles(FakeCommand)
    def do_something(self, cmd: FakeCommand) -> FakeEvent:
        if self._get_state().initialized:
            raise CommandRejectedError("Already initialized")
        return FakeEvent(result=f"processed:{cmd.value}")


class MultiEventAggregate(Aggregate[AggState]):
    """Aggregate that returns multiple events."""

    domain = "multi"

    def _create_empty_state(self) -> AggState:
        return AggState()

    def _apply_event(self, state: AggState, event_any) -> None:
        state.initialized = True

    @handles(FakeCommand)
    def do_multi(self, cmd: FakeCommand) -> tuple:
        return (FakeEvent(result="first"), FakeEvent(result="second"))


# =============================================================================
# Tests for @handles decorator
# =============================================================================


class TestHandlesDecorator:
    """Test @handles decorator."""

    def test_decorator_marks_handler(self):
        # Check the decorated method has the right attributes
        method = SampleAggregate.do_something
        assert hasattr(method, "_is_handler")
        assert method._is_handler is True
        assert method._command_type == FakeCommand

    def test_decorator_validates_missing_param(self):
        with pytest.raises(TypeError, match="must have cmd parameter"):
            @handles(FakeCommand)
            def bad_method(self):
                pass

    def test_decorator_validates_missing_type_hint(self):
        with pytest.raises(TypeError, match="missing type hint"):
            @handles(FakeCommand)
            def bad_method(self, cmd):
                pass

    def test_decorator_validates_type_hint_mismatch(self):
        with pytest.raises(TypeError, match="doesn't match type hint"):
            @handles(FakeCommand)
            def bad_method(self, cmd: AnotherCommand):
                pass

    def test_decorator_preserves_function_name(self):
        method = SampleAggregate.do_something
        assert method.__name__ == "do_something"


# =============================================================================
# Tests for Aggregate ABC
# =============================================================================


class TestAggregateInit:
    """Test Aggregate initialization."""

    def test_init_without_event_book(self):
        agg = SampleAggregate()
        eb = agg.event_book()
        assert len(eb.pages) == 0

    def test_init_with_event_book(self):
        prior_events = types.EventBook()
        event_any = any_pb2.Any(type_url="test.FakeEvent", value=b"prior")
        prior_events.pages.append(types.EventPage(event=event_any))

        agg = SampleAggregate(prior_events)
        # Access state to trigger rebuild
        state = agg._get_state()
        assert state.initialized is True

    def test_rebuild_clears_consumed_events(self):
        prior_events = types.EventBook()
        event_any = any_pb2.Any(type_url="test.FakeEvent", value=b"prior")
        prior_events.pages.append(types.EventPage(event=event_any))

        agg = SampleAggregate(prior_events)
        # Trigger rebuild
        agg._get_state()

        # Events are cleared after being consumed
        assert len(agg.event_book().pages) == 0


class TestAggregateDispatch:
    """Test Aggregate command dispatch."""

    def test_dispatch_finds_handler(self):
        agg = SampleAggregate()
        cmd_any = any_pb2.Any(type_url="test.FakeCommand", value=b"hello")
        agg.dispatch(cmd_any)

        # Event should be recorded
        assert len(agg.event_book().pages) == 1

    def test_dispatch_unknown_command(self):
        agg = SampleAggregate()
        cmd_any = any_pb2.Any(type_url="test.UnknownCommand", value=b"")

        with pytest.raises(ValueError, match="Unknown command"):
            agg.dispatch(cmd_any)

    def test_handler_can_reject(self):
        # First call succeeds
        agg = SampleAggregate()
        cmd_any = any_pb2.Any(type_url="test.FakeCommand", value=b"first")
        agg.dispatch(cmd_any)

        # Second call should be rejected
        with pytest.raises(CommandRejectedError, match="Already initialized"):
            agg.dispatch(cmd_any)


class TestAggregateMultiEvent:
    """Test aggregate returning multiple events."""

    def test_multi_event_records_all(self):
        agg = MultiEventAggregate()
        cmd_any = any_pb2.Any(type_url="test.FakeCommand", value=b"")
        agg.dispatch(cmd_any)

        eb = agg.event_book()
        assert len(eb.pages) == 2


class TestAggregateHandle:
    """Test classmethod handle() for gRPC integration."""

    def test_handle_creates_instance_and_dispatches(self):
        request = types.ContextualCommand(
            command=types.CommandBook(
                pages=[
                    types.CommandPage(
                        command=any_pb2.Any(
                            type_url="test.FakeCommand",
                            value=b"test_value",
                        ),
                    ),
                ],
            ),
        )

        response = SampleAggregate.handle(request)

        assert len(response.events.pages) == 1

    def test_handle_with_prior_events(self):
        prior = types.EventBook()
        prior.pages.append(
            types.EventPage(event=any_pb2.Any(type_url="test.FakeEvent", value=b""))
        )

        request = types.ContextualCommand(
            command=types.CommandBook(
                pages=[
                    types.CommandPage(
                        command=any_pb2.Any(
                            type_url="test.FakeCommand",
                            value=b"",
                        ),
                    ),
                ],
            ),
            events=prior,
        )

        # Should fail because state is already initialized
        with pytest.raises(CommandRejectedError, match="Already initialized"):
            SampleAggregate.handle(request)

    def test_handle_requires_command_pages(self):
        request = types.ContextualCommand(
            command=types.CommandBook(pages=[]),
        )

        with pytest.raises(ValueError, match="No command pages"):
            SampleAggregate.handle(request)


class TestAggregateApplyAndRecord:
    """Test _apply_and_record method."""

    def test_apply_and_record_packs_event(self):
        agg = SampleAggregate()
        agg._get_state()  # Initialize state
        event = FakeEvent(result="test")

        agg._apply_and_record(event)

        eb = agg.event_book()
        assert len(eb.pages) == 1
        assert "FakeEvent" in eb.pages[0].event.type_url

    def test_apply_and_record_updates_cached_state(self):
        agg = SampleAggregate()
        state = agg._get_state()
        assert state.initialized is False

        event = FakeEvent(result="test")
        agg._apply_and_record(event)

        # State should be updated
        assert state.initialized is True


# =============================================================================
# Tests for __init_subclass__ validation
# =============================================================================


class TestAggregateSubclassValidation:
    """Test aggregate subclass validation."""

    def test_missing_domain_raises(self):
        with pytest.raises(TypeError, match="must define 'domain'"):
            class BadAggregate(Aggregate[AggState]):
                def _create_empty_state(self):
                    return AggState()

                def _apply_event(self, state, event_any):
                    pass

    def test_duplicate_handler_raises(self):
        with pytest.raises(TypeError, match="duplicate handler"):
            class DuplicateAggregate(Aggregate[AggState]):
                domain = "dup"

                def _create_empty_state(self):
                    return AggState()

                def _apply_event(self, state, event_any):
                    pass

                @handles(FakeCommand)
                def handler_one(self, cmd: FakeCommand):
                    pass

                @handles(FakeCommand)
                def handler_two(self, cmd: FakeCommand):
                    pass


class TestAggregateStateCaching:
    """Test state caching behavior."""

    def test_state_lazily_rebuilt(self):
        agg = SampleAggregate()
        assert agg._state is None

        # Access state triggers rebuild
        state = agg._get_state()
        assert state is not None
        assert agg._state is state

    def test_state_cached_across_calls(self):
        agg = SampleAggregate()
        state1 = agg._get_state()
        state2 = agg._get_state()
        assert state1 is state2


# =============================================================================
# Tests for Aggregate.replay() classmethod
# =============================================================================


class TestAggregateReplay:
    """Test replay() classmethod for conflict detection."""

    def test_replay_empty_events(self):
        from angzarr_client.proto.angzarr import aggregate_pb2 as aggregate

        request = aggregate.ReplayRequest()
        response = SampleAggregate.replay(request)

        # Should return packed empty state
        assert response.state.type_url != ""
        assert "AggState" in response.state.type_url

    def test_replay_with_events(self):
        from angzarr_client.proto.angzarr import aggregate_pb2 as aggregate

        event_any = any_pb2.Any(type_url="test.FakeEvent", value=b"replay_value")

        request = aggregate.ReplayRequest(
            events=[types.EventPage(event=event_any)],
        )
        response = SampleAggregate.replay(request)

        # Should return state with event applied
        assert response.state.type_url != ""
        # Can't easily unpack fake state, but verify response is valid
        assert response.state.value != b""

    def test_replay_with_snapshot(self):
        from angzarr_client.proto.angzarr import aggregate_pb2 as aggregate

        # Create a snapshot with serialized state
        state = AggState(initialized=True, value="snapped")
        state_any = any_pb2.Any(
            type_url="type.googleapis.com/test.AggState",
            value=state.SerializeToString(),
        )

        snapshot = types.Snapshot(sequence=5, state=state_any)

        request = aggregate.ReplayRequest(
            base_snapshot=snapshot,
            events=[],
        )
        response = SampleAggregate.replay(request)

        # Should return state from snapshot
        assert response.state.type_url != ""
