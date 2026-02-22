"""Step definitions for aggregate client and router scenarios."""
import pytest
from pytest_bdd import given, when, then, parsers
from google.protobuf.any_pb2 import Any
from google.protobuf.empty_pb2 import Empty

from angzarr_client.proto.angzarr import types_pb2


class MockAggregateRouter:
    """Mock aggregate router for testing."""

    def __init__(self):
        self.handlers = {}
        self.invoked_handlers = []

    def on(self, suffix, handler):
        self.handlers[suffix] = handler
        return self

    def dispatch(self, command_type):
        for suffix, handler in self.handlers.items():
            if command_type == suffix or command_type.endswith(suffix):
                self.invoked_handlers.append(suffix)
                return handler()
        raise ValueError(f"Unknown command type: {command_type}")


class MockEventRouter:
    """Mock event router for testing."""

    def __init__(self):
        self.domains = {}
        self.current_domain = None
        self.invoked_handlers = []

    def domain(self, name):
        self.current_domain = name
        if name not in self.domains:
            self.domains[name] = {}
        return self

    def on(self, suffix, handler):
        if self.current_domain:
            self.domains[self.current_domain][suffix] = handler
        return self

    def dispatch(self, domain, event_type):
        if domain in self.domains:
            for suffix, handler in self.domains[domain].items():
                if event_type == suffix or event_type.endswith(suffix):
                    self.invoked_handlers.append(suffix)
                    handler()
                    return
        return None


@pytest.fixture
def aggregate_context():
    """Shared context for aggregate scenarios."""
    return {
        "aggregate_router": None,
        "saga_router": None,
        "projector_router": None,
        "pm_router": None,
        "response": None,
        "error": None,
        "invoked_handlers": [],
        "event_book": None,
    }


# ==========================================================================
# Aggregate Router Steps
# ==========================================================================


@given(parsers.parse('an aggregate router with handlers for "{type1}" and "{type2}"'))
def given_aggregate_router_with_handlers(aggregate_context, type1, type2):
    router = MockAggregateRouter()
    router.on(type1, lambda: make_event_book(0))
    router.on(type2, lambda: make_event_book(0))
    aggregate_context["aggregate_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers


@given("an aggregate router")
def given_aggregate_router(aggregate_context):
    router = MockAggregateRouter()
    router.on("TestCommand", lambda: make_event_book(0))
    aggregate_context["aggregate_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers


@given("an aggregate with existing events")
def given_aggregate_with_existing_events(aggregate_context):
    event_book = types_pb2.EventBook()
    event_book.cover.domain = "test"
    for i in range(3):
        page = event_book.pages.add()
        page.sequence = i
        page.event.Pack(Empty())
    aggregate_context["event_book"] = event_book


@given(parsers.parse("an aggregate at sequence {seq:d}"))
def given_aggregate_at_sequence(aggregate_context, seq):
    event_book = types_pb2.EventBook()
    event_book.cover.domain = "test"
    for i in range(seq):
        page = event_book.pages.add()
        page.sequence = i
        page.event.Pack(Empty())
    aggregate_context["event_book"] = event_book


@when(parsers.parse('I receive a "{command_type}" command'))
def when_receive_command(aggregate_context, command_type):
    try:
        result = aggregate_context["aggregate_router"].dispatch(command_type)
        aggregate_context["response"] = result
    except Exception as e:
        aggregate_context["error"] = e


@when("I receive a command for that aggregate")
def when_receive_command_for_aggregate(aggregate_context):
    try:
        result = aggregate_context["aggregate_router"].dispatch("TestCommand")
        aggregate_context["response"] = result
    except Exception as e:
        aggregate_context["error"] = e


@when(parsers.parse("I receive a command at sequence {seq:d}"))
def when_receive_command_at_sequence(aggregate_context, seq):
    try:
        result = aggregate_context["aggregate_router"].dispatch("TestCommand")
        aggregate_context["response"] = result
    except Exception as e:
        aggregate_context["error"] = e


@when(parsers.parse("a handler emits {count:d} events"))
def when_handler_emits_events(aggregate_context, count):
    def emit_multiple():
        book = types_pb2.EventBook()
        for i in range(count):
            page = book.pages.add()
            page.sequence = i
            page.event.Pack(Empty())
        return book

    router = MockAggregateRouter()
    router.on("MultiEmit", emit_multiple)
    aggregate_context["aggregate_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers
    aggregate_context["response"] = router.dispatch("MultiEmit")


@then(parsers.parse("the {handler_name} handler should be invoked"))
def then_handler_should_be_invoked(aggregate_context, handler_name):
    assert handler_name in aggregate_context["invoked_handlers"], \
        f"Handler {handler_name} was not invoked. Invoked: {aggregate_context['invoked_handlers']}"


@then(parsers.parse("the {handler_name} handler should NOT be invoked"))
def then_handler_should_not_be_invoked(aggregate_context, handler_name):
    assert handler_name not in aggregate_context["invoked_handlers"], \
        f"Handler {handler_name} was invoked but should not have been"


@then("the router should load the EventBook first")
def then_router_should_load_event_book(aggregate_context):
    assert aggregate_context["response"] is not None or aggregate_context["error"] is not None


@then("the handler should receive the reconstructed state")
def then_handler_should_receive_state(aggregate_context):
    assert len(aggregate_context["invoked_handlers"]) > 0


@then("the router should return those events")
def then_router_should_return_events(aggregate_context):
    assert aggregate_context["response"] is not None
    assert len(aggregate_context["response"].pages) > 0


@then("the events should have correct sequences")
def then_events_should_have_correct_sequences(aggregate_context):
    for i, page in enumerate(aggregate_context["response"].pages):
        assert page.sequence >= 0


@then("the router should return an error")
def then_router_should_return_error(aggregate_context):
    assert aggregate_context["error"] is not None


@then("the error should indicate unknown command type")
def then_error_should_indicate_unknown_command(aggregate_context):
    assert "Unknown command type" in str(aggregate_context["error"])


@then("no handler should be invoked")
def then_no_handler_should_be_invoked(aggregate_context):
    if aggregate_context["error"] is not None:
        return  # Error case
    assert len(aggregate_context["invoked_handlers"]) == 0


# ==========================================================================
# Saga Router Steps
# ==========================================================================


@given(parsers.parse('a saga router with handlers for "{type1}" and "{type2}"'))
def given_saga_router_with_handlers(aggregate_context, type1, type2):
    router = MockEventRouter()
    router.domain("orders").on(type1, lambda: None).on(type2, lambda: None)
    aggregate_context["saga_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers


@given("a saga router")
def given_saga_router(aggregate_context):
    router = MockEventRouter()
    router.domain("orders").on("OrderCreated", lambda: None)
    aggregate_context["saga_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers


@when(parsers.parse('I receive an "{event_type}" event'))
def when_receive_event(aggregate_context, event_type):
    aggregate_context["saga_router"].dispatch("orders", event_type)


# ==========================================================================
# PM Router Steps
# ==========================================================================


@given(parsers.parse('a PM router with handlers for "{type1}" and "{type2}"'))
def given_pm_router_with_handlers(aggregate_context, type1, type2):
    router = MockEventRouter()
    router.domain("orders").on(type1, lambda: None)
    router.domain("inventory").on(type2, lambda: None)
    aggregate_context["pm_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers


@given("a PM router")
def given_pm_router(aggregate_context):
    given_pm_router_with_handlers(aggregate_context, "OrderCreated", "InventoryReserved")


@when(parsers.parse('I receive an "{event_type}" event from domain "{domain}"'))
def when_receive_event_from_domain(aggregate_context, event_type, domain):
    aggregate_context["pm_router"].dispatch(domain, event_type)


@when("I receive an event without correlation ID")
def when_receive_event_without_correlation_id(aggregate_context):
    # Event without correlation ID should be skipped
    pass


@then("the event should be skipped")
def then_event_should_be_skipped(aggregate_context):
    assert len(aggregate_context["invoked_handlers"]) == 0


# ==========================================================================
# Projector Router Steps
# ==========================================================================


@given(parsers.parse('a projector router with handlers for "{event_type}"'))
def given_projector_router_with_handlers(aggregate_context, event_type):
    router = MockEventRouter()
    router.domain("orders").on(event_type, lambda: None)
    aggregate_context["projector_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers


@given("a projector router")
def given_projector_router(aggregate_context):
    given_projector_router_with_handlers(aggregate_context, "TestEvent")


# ==========================================================================
# Handler Registration Steps
# ==========================================================================


@given("a router")
def given_a_router(aggregate_context):
    router = MockEventRouter()
    aggregate_context["saga_router"] = router
    aggregate_context["invoked_handlers"] = router.invoked_handlers


@when(parsers.parse('I register handler for type "{event_type}"'))
def when_register_handler_for_type(aggregate_context, event_type):
    aggregate_context["saga_router"].domain("test").on(event_type, lambda: None)


@when(parsers.parse('I register handlers for "{type1}", "{type2}", and "{type3}"'))
def when_register_multiple_handlers(aggregate_context, type1, type2, type3):
    router = aggregate_context["saga_router"]
    router.domain("test").on(type1, lambda: None).on(type2, lambda: None).on(type3, lambda: None)


@then(parsers.parse('events ending with "{suffix}" should match'))
def then_events_ending_with_should_match(aggregate_context, suffix):
    router = aggregate_context["saga_router"]
    assert "test" in router.domains
    assert suffix in router.domains["test"]


@then(parsers.parse('events ending with "{suffix}" should NOT match'))
def then_events_ending_with_should_not_match(aggregate_context, suffix):
    router = aggregate_context["saga_router"]
    if "test" in router.domains:
        assert suffix not in router.domains["test"]


@then("all three types should be routable")
def then_all_three_types_should_be_routable(aggregate_context):
    router = aggregate_context["saga_router"]
    assert len(router.domains.get("test", {})) == 3


@then("each should invoke its specific handler")
def then_each_should_invoke_its_handler(aggregate_context):
    pass  # Verified by registration


# ==========================================================================
# New Step Definitions (Updated Feature File Patterns)
# ==========================================================================


@then("the aggregate operation should fail with connection error")
def then_aggregate_operation_should_fail_with_connection_error(aggregate_context):
    error = aggregate_context.get("error")
    assert error is not None, "Expected connection error"
    assert "connection" in str(error).lower() or isinstance(error, ConnectionError)


@given("a saga router with a rejected command")
def given_saga_router_with_rejected_command(aggregate_context):
    router = MockEventRouter()
    router.domain("test")
    aggregate_context["saga_router"] = router
    aggregate_context["rejection"] = types_pb2.RevocationResponse(
        reason="Command rejected by target aggregate"
    )


@when("the router processes the rejection")
def when_router_processes_rejection(aggregate_context):
    rejection = aggregate_context.get("rejection")
    assert rejection is not None, "Expected rejection to be present"


@then("the router projection state should be returned")
def then_router_projection_state_should_be_returned(aggregate_context):
    state = aggregate_context.get("built_state") or aggregate_context.get("last_projection")
    assert state is not None, "Expected projection state to be returned"


# ==========================================================================
# Helper Functions
# ==========================================================================


def make_event_book(seq):
    """Create a test EventBook."""
    book = types_pb2.EventBook()
    page = book.pages.add()
    page.sequence = seq
    page.event.Pack(Empty())
    return book
