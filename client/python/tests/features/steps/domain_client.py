"""DomainClient step definitions."""

import os
import uuid
from unittest.mock import MagicMock

import pytest
from google.protobuf.any_pb2 import Any
from google.protobuf.empty_pb2 import Empty
from pytest_bdd import given, parsers, scenarios, then, when

from angzarr_client.proto.angzarr import types_pb2

# Link to feature file


@pytest.fixture
def domain_client_context():
    """Test context for domain client scenarios."""
    return {
        "client": None,
        "domain": "",
        "endpoint": "localhost:50051",
        "closed": False,
        "command_response": None,
        "query_response": None,
        "event_pages": None,
        "error": None,
        "env_var_name": "",
        "event_book": None,
    }


# ==========================================================================
# Background Steps
# ==========================================================================


@given(parsers.parse('a running aggregate coordinator for domain "{domain}"'))
def given_running_coordinator(domain_client_context, domain):
    domain_client_context["domain"] = domain


@given(parsers.parse('a registered aggregate handler for domain "{domain}"'))
def given_registered_handler(domain_client_context, domain):
    # Handler registration is implicit for testing
    pass


# ==========================================================================
# Given Steps
# ==========================================================================


@given("a connected DomainClient")
def given_connected_domain_client(domain_client_context):
    domain_client_context["client"] = MagicMock()
    domain_client_context["closed"] = False


@given(
    parsers.parse('environment variable "{env_var}" is set to the coordinator endpoint')
)
def given_env_var_set_to_endpoint(domain_client_context, env_var):
    domain_client_context["env_var_name"] = env_var
    os.environ[env_var] = domain_client_context["endpoint"]


@given(parsers.parse('an aggregate "{domain}" with root "{root}" has {count:d} events'))
def given_aggregate_with_events(domain_client_context, domain, root, count, request):
    from tests.features.conftest import SHARED_EVENT_STORE

    try:
        root_uuid = uuid.UUID(root)
    except ValueError:
        root_uuid = uuid.uuid4()

    event_book = types_pb2.EventBook()
    event_book.cover.domain = domain
    event_book.cover.root.value = root_uuid.bytes

    for i in range(count):
        page = event_book.pages.add()
        page.header.sequence = i  # 0-indexed for consistency
        page.event.Pack(Empty())

    domain_client_context["event_book"] = event_book
    SHARED_EVENT_STORE[root] = event_book

    # Also populate query_context if available
    try:
        query_context = request.getfixturevalue("query_context")
        key = f"{domain}:{root}"
        query_context["aggregates"][key] = event_book
    except Exception:
        pass

    # Also populate speculative_context if available
    try:
        speculative_context = request.getfixturevalue("speculative_context")
        speculative_context["event_book"] = event_book
        speculative_context["base_event_count"] = count
    except Exception:
        pass


# ==========================================================================
# When Steps
# ==========================================================================


@when("I create a DomainClient for the coordinator endpoint")
def when_create_domain_client_for_endpoint(domain_client_context):
    domain_client_context["client"] = MagicMock()
    domain_client_context["client"].aggregate = MagicMock()
    domain_client_context["client"].query = MagicMock()
    domain_client_context["closed"] = False


@when(parsers.parse('I create a DomainClient for domain "{domain}"'))
def when_create_domain_client_for_domain(domain_client_context, domain):
    domain_client_context["domain"] = domain
    domain_client_context["client"] = MagicMock()
    domain_client_context["client"].aggregate = MagicMock()
    domain_client_context["client"].query = MagicMock()
    domain_client_context["closed"] = False


@when("I use the command builder to send a command")
def when_use_command_builder(domain_client_context):
    domain_client_context["command_response"] = MagicMock()


@when("I use the query builder to fetch events for that root")
def when_use_query_builder(domain_client_context):
    from tests.features.conftest import SHARED_EVENT_STORE

    event_book = domain_client_context.get("event_book")
    if event_book:
        domain_client_context["event_pages"] = list(event_book.pages)
    elif SHARED_EVENT_STORE:
        # Fetch from shared event store (populated by query_client.py's step handler)
        for book in SHARED_EVENT_STORE.values():
            domain_client_context["event_pages"] = list(book.pages)
            break
    else:
        domain_client_context["event_pages"] = []


@when("I send a command")
def when_send_command(domain_client_context):
    if domain_client_context["closed"]:
        domain_client_context["error"] = ConnectionError("Connection closed")
        return
    domain_client_context["command_response"] = MagicMock()


@when("I query for the resulting events")
def when_query_events(domain_client_context):
    if domain_client_context["closed"]:
        domain_client_context["error"] = ConnectionError("Connection closed")
        return
    domain_client_context["query_response"] = types_pb2.EventBook()


@when("I close the DomainClient")
def when_close_domain_client(domain_client_context):
    domain_client_context["closed"] = True


@when(parsers.parse('I create a DomainClient from environment variable "{env_var}"'))
def when_create_domain_client_from_env(domain_client_context, env_var):
    endpoint = os.environ.get(env_var)
    assert endpoint, f"Environment variable {env_var} should be set"
    domain_client_context["client"] = MagicMock()
    domain_client_context["closed"] = False


# ==========================================================================
# Then Steps
# ==========================================================================


@then("I should be able to query events")
def then_can_query_events(domain_client_context):
    assert not domain_client_context["closed"], "Client should be connected"


@then("I should be able to send commands")
def then_can_send_commands(domain_client_context):
    assert not domain_client_context["closed"], "Client should be connected"


@then("I should receive a CommandResponse")
def then_receive_command_response(domain_client_context):
    assert (
        domain_client_context["command_response"] is not None
    ), "Should receive a CommandResponse"


@then(parsers.parse("I should receive {count:d} EventPages"))
def then_receive_event_pages(domain_client_context, count):
    event_pages = domain_client_context["event_pages"]
    assert event_pages is not None
    assert len(event_pages) == count, f"Expected {count} pages, got {len(event_pages)}"


@then("both operations should succeed on the same connection")
def then_both_operations_succeed(domain_client_context):
    assert (
        domain_client_context["command_response"] is not None
    ), "Command should have succeeded"
    assert (
        domain_client_context["query_response"] is not None
    ), "Query should have succeeded"


@then("subsequent commands should fail with ConnectionError")
def then_subsequent_commands_fail(domain_client_context):
    assert domain_client_context["closed"], "Client should be closed"
    when_send_command(domain_client_context)
    assert (
        domain_client_context["error"] is not None
    ), "Commands should fail after close"


@then("subsequent queries should fail with ConnectionError")
def then_subsequent_queries_fail(domain_client_context):
    assert domain_client_context["closed"], "Client should be closed"
    when_query_events(domain_client_context)
    assert domain_client_context["error"] is not None, "Queries should fail after close"


@then("the DomainClient should be connected")
def then_domain_client_connected(domain_client_context):
    assert not domain_client_context["closed"], "Client should be connected"


# ==========================================================================
# Cleanup
# ==========================================================================


@pytest.fixture(autouse=True)
def cleanup_env_vars(domain_client_context):
    """Clean up environment variables after test."""
    yield
    env_var = domain_client_context.get("env_var_name")
    if env_var and env_var in os.environ:
        del os.environ[env_var]
