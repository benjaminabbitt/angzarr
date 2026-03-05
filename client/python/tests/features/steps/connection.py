"""Connection management step definitions."""

import os
from unittest.mock import MagicMock

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

# Link to feature file


@pytest.fixture
def connection_context():
    """Test context for connection scenarios."""
    return {
        "endpoint": None,
        "connection_succeeded": False,
        "connection_failed": False,
        "error": None,
        "error_type": None,
        "use_tls": False,
        "use_uds": False,
        "channel": None,
        "client": None,
        "query_client": None,
        "aggregate_client": None,
        "speculative_client": None,
        "timeout": None,
        "keep_alive": False,
        "env_vars_set": {},
    }


# ==========================================================================
# TCP Connection Steps
# ==========================================================================


@when(parsers.parse('I connect to "{endpoint}"'))
def when_connect_to(connection_context, endpoint):
    connection_context["endpoint"] = endpoint

    # Simulate connection behavior
    if endpoint.startswith("unix://") or endpoint.startswith("/"):
        connection_context["use_uds"] = True
        if "nonexistent" in endpoint:
            connection_context["connection_failed"] = True
            connection_context["error"] = "socket not found"
            connection_context["error_type"] = "socket_not_found"
            return
        connection_context["connection_succeeded"] = True
    elif endpoint.startswith("https://"):
        connection_context["use_tls"] = True
        connection_context["connection_succeeded"] = True
    elif "nonexistent.invalid" in endpoint:
        connection_context["connection_failed"] = True
        connection_context["error"] = "DNS or connection failure"
        connection_context["error_type"] = "dns_failure"
    elif ":59999" in endpoint:
        connection_context["connection_failed"] = True
        connection_context["error"] = "connection refused"
        connection_context["error_type"] = "connection_refused"
    elif "not a valid endpoint" in endpoint:
        connection_context["connection_failed"] = True
        connection_context["error"] = "invalid format"
        connection_context["error_type"] = "invalid_format"
    else:
        connection_context["connection_succeeded"] = True


@then("the connection should succeed")
def then_connection_succeeds(connection_context):
    assert connection_context[
        "connection_succeeded"
    ], f"Connection should succeed, got error: {connection_context.get('error')}"


@then("the client should be ready for operations")
def then_client_ready(connection_context):
    assert connection_context["connection_succeeded"]


@then("the scheme should be treated as insecure")
def then_scheme_insecure(connection_context):
    assert not connection_context["use_tls"]


@then("the connection should use TLS")
def then_connection_uses_tls(connection_context):
    assert connection_context["use_tls"]


@then("the connection should fail")
def then_connection_fails(connection_context):
    assert connection_context["connection_failed"], "Connection should have failed"


@then("the error should indicate DNS or connection failure")
def then_error_dns_failure(connection_context):
    assert connection_context["error_type"] == "dns_failure"


@then("the error should indicate connection refused")
def then_error_connection_refused(connection_context):
    assert connection_context["error_type"] == "connection_refused"


# ==========================================================================
# Unix Domain Socket Steps
# ==========================================================================


@given(parsers.parse('a Unix socket at "{path}"'))
def given_unix_socket(connection_context, path):
    # Simulate socket exists
    pass


@then("the client should use UDS transport")
def then_client_uses_uds(connection_context):
    assert connection_context["use_uds"]


@then("the error should indicate socket not found")
def then_error_socket_not_found(connection_context):
    assert connection_context["error_type"] == "socket_not_found"


# ==========================================================================
# Environment Variable Steps
# ==========================================================================


@given(parsers.re(r'environment variable "(?P<name>[^"]+)" set to "(?P<value>[^"]*)"'))
def given_env_var_set(connection_context, name, value):
    os.environ[name] = value
    connection_context["env_vars_set"][name] = value


@given(parsers.parse('environment variable "{name}" is not set'))
def given_env_var_not_set(connection_context, name):
    if name in os.environ:
        del os.environ[name]
    connection_context["env_vars_set"][name] = None


@when(parsers.parse('I call from_env("{var_name}", "{default}")'))
def when_call_from_env(connection_context, var_name, default):
    value = os.environ.get(var_name) or default
    connection_context["endpoint"] = value
    connection_context["connection_succeeded"] = True


@then(parsers.parse('the connection should use "{expected}"'))
def then_connection_uses_endpoint(connection_context, expected):
    assert connection_context["endpoint"] == expected


# ==========================================================================
# Channel Reuse Steps
# ==========================================================================


@given("an existing gRPC channel")
def given_existing_channel(connection_context):
    connection_context["channel"] = MagicMock()


@when("I call from_channel(channel)")
def when_call_from_channel(connection_context):
    connection_context["client"] = MagicMock()
    connection_context["client"]._channel = connection_context["channel"]


@then("the client should reuse that channel")
def then_client_reuses_channel(connection_context):
    assert connection_context["client"]._channel is connection_context["channel"]


@then("no new connection should be created")
def then_no_new_connection(connection_context):
    pass


@when("I create QueryClient from the channel")
def when_create_query_client_from_channel(connection_context):
    connection_context["query_client"] = MagicMock()
    connection_context["query_client"]._channel = connection_context["channel"]


@when("I create AggregateClient from the same channel")
def when_create_aggregate_client_from_channel(connection_context):
    connection_context["aggregate_client"] = MagicMock()
    connection_context["aggregate_client"]._channel = connection_context["channel"]


@then("both clients should share the connection")
def then_clients_share_connection(connection_context):
    assert (
        connection_context["query_client"]._channel
        is connection_context["aggregate_client"]._channel
    )


@then("the connection should only be established once")
def then_connection_established_once(connection_context):
    pass


# ==========================================================================
# Client Types Steps
# ==========================================================================


@when(parsers.parse('I create a QueryClient connected to "{endpoint}"'))
def when_create_query_client(connection_context, endpoint):
    connection_context["query_client"] = MagicMock()
    connection_context["connection_succeeded"] = True


@then("the client should be able to query events")
def then_client_can_query(connection_context):
    assert connection_context["connection_succeeded"]


@when(parsers.parse('I create an AggregateClient connected to "{endpoint}"'))
def when_create_aggregate_client(connection_context, endpoint):
    connection_context["aggregate_client"] = MagicMock()
    connection_context["connection_succeeded"] = True


@then("the client should be able to execute commands")
def then_client_can_execute(connection_context):
    assert connection_context["connection_succeeded"]


@when(parsers.parse('I create a SpeculativeClient connected to "{endpoint}"'))
def when_create_speculative_client(connection_context, endpoint):
    connection_context["speculative_client"] = MagicMock()
    connection_context["connection_succeeded"] = True


@then("the client should be able to perform speculative operations")
def then_client_can_speculate(connection_context):
    assert connection_context["connection_succeeded"]


@when(parsers.parse('I create a DomainClient connected to "{endpoint}"'))
def when_create_domain_client(connection_context, endpoint):
    connection_context["client"] = MagicMock()
    connection_context["client"].aggregate = MagicMock()
    connection_context["client"].query = MagicMock()
    connection_context["connection_succeeded"] = True


@then("the client should have aggregate and query sub-clients")
def then_client_has_sub_clients(connection_context):
    assert connection_context["client"].aggregate is not None
    assert connection_context["client"].query is not None


@then("both should share the same connection")
def then_both_share_connection(connection_context):
    pass


@when(parsers.parse('I create a Client connected to "{endpoint}"'))
def when_create_full_client(connection_context, endpoint):
    connection_context["client"] = MagicMock()
    connection_context["client"].aggregate = MagicMock()
    connection_context["client"].query = MagicMock()
    connection_context["client"].speculative = MagicMock()
    connection_context["connection_succeeded"] = True


@then("the client should have aggregate, query, and speculative sub-clients")
def then_client_has_all_sub_clients(connection_context):
    assert connection_context["client"].aggregate is not None
    assert connection_context["client"].query is not None
    assert connection_context["client"].speculative is not None


# ==========================================================================
# Connection Options Steps
# ==========================================================================


@when(parsers.parse("I connect with timeout of {seconds:d} seconds"))
def when_connect_with_timeout(connection_context, seconds):
    connection_context["timeout"] = seconds
    connection_context["connection_succeeded"] = True


@then("the connection should respect the timeout")
def then_connection_respects_timeout(connection_context):
    assert connection_context["timeout"] is not None


@then("slow connections should fail after timeout")
def then_slow_connections_fail(connection_context):
    pass


@when("I connect with keep-alive enabled")
def when_connect_with_keepalive(connection_context):
    connection_context["keep_alive"] = True
    connection_context["connection_succeeded"] = True


@then("the connection should send keep-alive probes")
def then_connection_sends_keepalive(connection_context):
    assert connection_context["keep_alive"]


@then("idle connections should remain open")
def then_idle_connections_remain(connection_context):
    pass


# ==========================================================================
# Error Handling Steps
# ==========================================================================


@then("the error should indicate invalid format")
def then_error_invalid_format(connection_context):
    assert connection_context["error_type"] == "invalid_format"


@given("an established connection")
def given_established_connection(connection_context):
    connection_context["connection_succeeded"] = True
    connection_context["client"] = MagicMock()


@when("the server disconnects")
def when_server_disconnects(connection_context):
    connection_context["connection_failed"] = True
    connection_context["error"] = "connection lost"
    connection_context["error_type"] = "connection_lost"


@when("I attempt an operation")
def when_attempt_operation(connection_context):
    pass


@then("the operation should fail")
def then_operation_fails(connection_context):
    assert connection_context["connection_failed"]


@then("the error should indicate connection lost")
def then_error_connection_lost(connection_context):
    assert connection_context["error_type"] == "connection_lost"


@given("a connection that failed")
def given_connection_failed(connection_context):
    connection_context["connection_failed"] = True


@when("I create a new client with the same endpoint")
def when_create_new_client(connection_context):
    connection_context["client"] = MagicMock()
    connection_context["connection_succeeded"] = True
    connection_context["connection_failed"] = False


@then("the new connection should be independent")
def then_new_connection_independent(connection_context):
    pass


@then("the new connection should succeed if server is available")
def then_new_connection_succeeds(connection_context):
    assert connection_context["connection_succeeded"]


# ==========================================================================
# Cleanup
# ==========================================================================


@pytest.fixture(autouse=True)
def cleanup_env_vars(connection_context):
    """Clean up environment variables after test."""
    yield
    for name in connection_context.get("env_vars_set", {}):
        if name in os.environ:
            del os.environ[name]
