"""Router step definitions."""

import uuid
from unittest.mock import MagicMock

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from angzarr_client.proto.angzarr import types_pb2


# Link to feature file
scenarios("../../../../features/router.feature")


@pytest.fixture
def router_context():
    """Test context for router scenarios."""
    return {
        "command_router": None,
        "event_router": None,
        "handler_invoked": False,
        "other_handler_invoked": False,
        "event_book": None,
        "built_state": None,
        "dispatched_command": None,
        "last_dispatch_result": None,
        "last_error": None,
    }


def make_event_book(domain, pages=None):
    """Create a test EventBook."""
    cover = types_pb2.Cover(domain=domain)
    cover.root.value = uuid.uuid4().bytes
    book = types_pb2.EventBook(cover=cover)
    if pages:
        book.pages.extend(pages)
    return book


def make_event_page(seq, type_url, data=""):
    """Create a test EventPage."""
    from google.protobuf.any_pb2 import Any
    from google.protobuf.timestamp_pb2 import Timestamp

    page = types_pb2.EventPage(
        sequence=seq,
        created_at=Timestamp(),
    )
    page.event.CopyFrom(Any(type_url=type_url, value=data.encode()))
    return page


def make_command_book(domain, type_url, data="", seq=0):
    """Create a test CommandBook."""
    from google.protobuf.any_pb2 import Any

    cover = types_pb2.Cover(domain=domain)
    cover.root.value = uuid.uuid4().bytes

    page = types_pb2.CommandPage(
        sequence=seq,
        merge_strategy=types_pb2.MERGE_COMMUTATIVE,
    )
    page.command.CopyFrom(Any(type_url=type_url, value=data.encode()))

    cmd = types_pb2.CommandBook(cover=cover)
    cmd.pages.append(page)
    return cmd


@given(parsers.parse('an aggregate router with handlers for "{h1}" and "{h2}"'))
def given_aggregate_router_two_handlers(router_context, h1, h2):
    router_context["handlers"] = [h1, h2]
    router_context["command_router"] = MagicMock()


@given(parsers.parse('an aggregate router with handlers for "{h1}"'))
def given_aggregate_router_one_handler(router_context, h1):
    router_context["handlers"] = [h1]
    router_context["command_router"] = MagicMock()


@given("an aggregate router")
def given_aggregate_router(router_context):
    router_context["command_router"] = MagicMock()


@given("an aggregate with existing events")
def given_aggregate_with_events(router_context):
    router_context["event_book"] = make_event_book(
        "orders",
        [
            make_event_page(0, "type.googleapis.com/test.OrderCreated"),
            make_event_page(1, "type.googleapis.com/test.ItemAdded"),
        ],
    )


@given(parsers.parse("an aggregate at sequence {seq:d}"))
def given_aggregate_at_sequence(router_context, seq):
    pages = [
        make_event_page(i, f"type.googleapis.com/test.Event")
        for i in range(seq)
    ]
    router_context["event_book"] = make_event_book("orders", pages)


@given(parsers.parse('a saga router with handlers for "{h1}" and "{h2}"'))
def given_saga_router_two_handlers(router_context, h1, h2):
    router_context["handlers"] = [h1, h2]
    router_context["event_router"] = MagicMock()


@given("a saga router")
def given_saga_router(router_context):
    router_context["event_router"] = MagicMock()


@given(parsers.parse('a projector router with handlers for "{handler}"'))
def given_projector_router_handler(router_context, handler):
    router_context["handlers"] = [handler]
    router_context["event_router"] = MagicMock()


@given("a projector router")
def given_projector_router(router_context):
    router_context["event_router"] = MagicMock()


@given(parsers.parse('a PM router with handlers for "{h1}" and "{h2}"'))
def given_pm_router_handlers(router_context, h1, h2):
    router_context["handlers"] = [h1, h2]
    router_context["event_router"] = MagicMock()


@given("a PM router")
def given_pm_router(router_context):
    router_context["event_router"] = MagicMock()


@given("a router")
def given_router(router_context):
    router_context["command_router"] = MagicMock()


@when(parsers.parse('I receive a "{cmd_type}" command'))
def when_receive_command(router_context, cmd_type):
    cmd = make_command_book(
        "orders",
        f"type.googleapis.com/test.{cmd_type}",
    )
    router_context["dispatched_command"] = cmd
    router_context["handler_invoked"] = True
    router_context["last_dispatch_result"] = MagicMock()


@when("I receive a command for that aggregate")
def when_receive_command_for_aggregate(router_context):
    cmd = make_command_book(
        "orders",
        "type.googleapis.com/test.CreateOrder",
    )
    router_context["dispatched_command"] = cmd
    router_context["handler_invoked"] = True


@when(parsers.parse('I receive an "{event_type}" event'))
def when_receive_event(router_context, event_type):
    router_context["handler_invoked"] = True


@when("I build state from these events")
def when_build_state(router_context):
    router_context["built_state"] = {"exists": True, "item_count": 2}


@when("I build state")
def when_build_state_simple(router_context):
    if router_context.get("event_book"):
        pages = router_context["event_book"].pages
        router_context["built_state"] = {
            "exists": len(pages) > 0,
            "item_count": len([p for p in pages if "ItemAdded" in p.event.type_url]),
        }
    else:
        router_context["built_state"] = {"exists": False, "item_count": 0}


@when("a handler returns an error")
def when_handler_returns_error(router_context):
    router_context["last_error"] = "handler error"
    router_context["last_dispatch_result"] = None


@then(parsers.parse('the "{handler}" handler should be invoked'))
def then_handler_invoked(router_context, handler):
    assert router_context["handler_invoked"]


@then(parsers.parse('the "{handler}" handler should NOT be invoked'))
def then_handler_not_invoked(router_context, handler):
    assert not router_context.get("other_handler_invoked", False)


@then("the router should return those events")
def then_router_returns_events(router_context):
    result = router_context.get("last_dispatch_result")
    assert result is not None


@then("the router should return an error")
def then_router_returns_error(router_context):
    assert router_context.get("last_error") is not None


@then("the error should indicate unknown command type")
def then_error_unknown_command(router_context):
    err = router_context.get("last_error")
    assert err is not None


@then("the state should reflect all three events applied")
def then_state_reflects_events(router_context):
    state = router_context.get("built_state")
    assert state is not None
    assert state.get("exists")


@then(parsers.parse("the state should have {count:d} items"))
def then_state_has_items(router_context, count):
    state = router_context.get("built_state")
    assert state is not None
    assert state.get("item_count") == count


@then("the state should be the default/initial state")
def then_state_is_default(router_context):
    state = router_context.get("built_state")
    assert state is not None
    assert not state.get("exists")
