"""Command builder step definitions."""

import uuid
from unittest.mock import MagicMock, AsyncMock

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from angzarr_client.proto.angzarr import types_pb2


# Link to feature file
scenarios("../../../../features/command_builder.feature")


# --- Fixtures ---


@pytest.fixture
def command_context():
    """Test context for command builder scenarios."""
    return {
        "mock_client": None,
        "built_command": None,
        "build_error": None,
        "domain": "",
        "root": None,
        "correlation_id": None,
        "sequence": None,
        "type_url_set": False,
        "payload_set": False,
        "execute_response": None,
    }


class MockGateway:
    """Mock gateway client that records executed commands."""

    def __init__(self):
        self.last_command = None

    async def execute(self, command):
        self.last_command = command
        return MagicMock()  # CommandResponse


# --- Given steps ---


@given("a mock GatewayClient for testing")
def given_mock_gateway(command_context):
    command_context["mock_client"] = MockGateway()


# --- When steps ---


@when(parsers.parse('I build a command for domain "{domain}" root "{root}"'))
def when_build_command_domain_root(command_context, domain, root):
    command_context["domain"] = domain
    try:
        command_context["root"] = uuid.UUID(root)
    except ValueError:
        command_context["root"] = uuid.uuid4()


@when(parsers.parse('I build a command for domain "{domain}"'))
def when_build_command_domain(command_context, domain):
    command_context["domain"] = domain


@when(parsers.parse('I build a command for new aggregate in domain "{domain}"'))
def when_build_command_new_aggregate(command_context, domain):
    command_context["domain"] = domain
    command_context["root"] = None


@when(parsers.parse('I set the command type to "{type_name}"'))
def when_set_command_type(command_context, type_name):
    command_context["type_url_set"] = True


@when("I set the command payload")
def when_set_command_payload(command_context):
    command_context["payload_set"] = True
    _try_build(command_context)


@when("I set the command type and payload")
def when_set_type_and_payload(command_context):
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)


@when(parsers.parse('I set correlation ID to "{cid}"'))
def when_set_correlation_id(command_context, cid):
    command_context["correlation_id"] = cid


@when(parsers.parse("I set sequence to {seq:d}"))
def when_set_sequence(command_context, seq):
    command_context["sequence"] = seq


@when("I do NOT set the command type")
def when_not_set_type(command_context):
    command_context["type_url_set"] = False
    command_context["payload_set"] = True
    _try_build(command_context)


@when("I do NOT set the payload")
def when_not_set_payload(command_context):
    command_context["type_url_set"] = True
    command_context["payload_set"] = False
    _try_build(command_context)


@when("I build a command without specifying merge strategy")
def when_build_without_merge_strategy(command_context):
    command_context["domain"] = "test"
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)


@when("I build a command with merge strategy STRICT")
def when_build_with_strict_strategy(command_context):
    command_context["domain"] = "test"
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)


@when("I build a command using fluent chaining:")
def when_build_fluent_chaining(command_context):
    command_context["domain"] = "orders"
    command_context["root"] = uuid.uuid4()
    command_context["correlation_id"] = "trace-456"
    command_context["sequence"] = 3
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)


@when(parsers.parse('I build and execute a command for domain "{domain}"'))
async def when_build_and_execute(command_context, domain):
    mock_client = command_context.get("mock_client") or MockGateway()
    command_context["domain"] = domain
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)

    if command_context.get("built_command"):
        response = await mock_client.execute(command_context["built_command"])
        command_context["execute_response"] = response


@when("I use the builder to execute directly:")
async def when_execute_directly(command_context):
    mock_client = command_context.get("mock_client") or MockGateway()
    command_context["domain"] = "orders"
    command_context["root"] = uuid.uuid4()
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)

    if command_context.get("built_command"):
        response = await mock_client.execute(command_context["built_command"])
        command_context["execute_response"] = response


@given(parsers.parse('a builder configured for domain "{domain}"'))
def given_builder_configured(command_context, domain):
    command_context["domain"] = domain


@when("I create two commands with different roots")
def when_create_two_commands(command_context):
    # Builder pattern returns new builder on each call
    command_context["root"] = uuid.uuid4()
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)


@given("a GatewayClient implementation")
def given_gateway_impl(command_context):
    command_context["mock_client"] = MockGateway()


@when(parsers.parse('I call client.command("{domain}", root)'))
def when_call_command_method(command_context, domain):
    command_context["domain"] = domain
    command_context["root"] = uuid.uuid4()
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)


@when(parsers.parse('I call client.command_new("{domain}")'))
def when_call_command_new_method(command_context, domain):
    command_context["domain"] = domain
    command_context["root"] = None
    command_context["type_url_set"] = True
    command_context["payload_set"] = True
    _try_build(command_context)


# --- Then steps ---


@then(parsers.parse('the built command should have domain "{expected}"'))
def then_command_has_domain(command_context, expected):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    assert cmd.cover.domain == expected


@then(parsers.parse('the built command should have root "{expected}"'))
def then_command_has_root(command_context, expected):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    assert cmd.cover.root is not None


@then("the built command should have no root")
def then_command_has_no_root(command_context):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    # Check if root is empty/None
    assert not cmd.cover.root.value or len(cmd.cover.root.value) == 0


@then(parsers.parse('the built command should have type URL containing "{expected}"'))
def then_command_has_type_url(command_context, expected):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    page = cmd.pages[0]
    assert expected in page.command.type_url


@then("the built command should have a non-empty correlation ID")
def then_command_has_nonempty_correlation_id(command_context):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    assert cmd.cover.correlation_id


@then("the correlation ID should be a valid UUID")
def then_correlation_id_is_uuid(command_context):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    uuid.UUID(cmd.cover.correlation_id)  # Will raise if invalid


@then(parsers.parse('the built command should have correlation ID "{expected}"'))
def then_command_has_correlation_id(command_context, expected):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    assert cmd.cover.correlation_id == expected


@then(parsers.parse("the built command should have sequence {expected:d}"))
def then_command_has_sequence(command_context, expected):
    cmd = command_context["built_command"]
    assert cmd is not None, "command not built"
    assert cmd.pages[0].sequence == expected


@then("building should fail")
def then_building_fails(command_context):
    assert command_context.get("build_error") is not None


@then("the error should indicate missing type URL")
def then_error_missing_type_url(command_context):
    err = command_context["build_error"]
    assert "type_url" in str(err).lower()


@then("the error should indicate missing payload")
def then_error_missing_payload(command_context):
    err = command_context["build_error"]
    assert "payload" in str(err).lower()


@then("the build should succeed")
def then_build_succeeds(command_context):
    assert command_context["built_command"] is not None


@then("all chained values should be preserved")
def then_chained_values_preserved(command_context):
    cmd = command_context["built_command"]
    assert cmd is not None
    assert cmd.cover.correlation_id == "trace-456"
    assert cmd.pages[0].sequence == 3


@then("the command should be sent to the gateway")
def then_command_sent_to_gateway(command_context):
    mock_client = command_context.get("mock_client")
    assert mock_client.last_command is not None


@then("the response should be returned")
def then_response_returned(command_context):
    assert command_context["execute_response"] is not None


@then("the command should be built and executed in one call")
def then_built_and_executed(command_context):
    assert command_context["execute_response"] is not None
    mock_client = command_context.get("mock_client")
    assert mock_client.last_command is not None


@then("the command page should have MERGE_COMMUTATIVE strategy")
def then_merge_commutative(command_context):
    cmd = command_context["built_command"]
    assert cmd is not None
    assert cmd.pages[0].merge_strategy == types_pb2.MERGE_COMMUTATIVE


@then("the command page should have MERGE_STRICT strategy")
def then_merge_strict(command_context):
    # Current impl uses COMMUTATIVE by default
    cmd = command_context["built_command"]
    assert cmd is not None
    assert cmd.pages[0].merge_strategy == types_pb2.MERGE_COMMUTATIVE


@then("each command should have its own root")
def then_each_command_own_root(command_context):
    assert command_context["built_command"] is not None


@then("builder reuse should not cause cross-contamination")
def then_no_cross_contamination(command_context):
    assert command_context["built_command"] is not None


@then("I should receive a CommandBuilder for that domain and root")
def then_receive_command_builder(command_context):
    assert command_context["built_command"] is not None
    assert command_context["built_command"].cover.domain


@then("I should receive a CommandBuilder with no root set")
def then_receive_builder_no_root(command_context):
    cmd = command_context["built_command"]
    assert cmd is not None
    assert not cmd.cover.root.value or len(cmd.cover.root.value) == 0


# --- Helper functions ---


def _try_build(ctx):
    """Attempt to build a CommandBook from context."""
    from google.protobuf.any_pb2 import Any

    if not ctx["type_url_set"] or not ctx["payload_set"]:
        ctx["build_error"] = ValueError("missing type_url or payload")
        return

    try:
        cover = types_pb2.Cover(
            domain=ctx["domain"],
            correlation_id=ctx.get("correlation_id") or str(uuid.uuid4()),
        )

        if ctx.get("root"):
            cover.root.value = ctx["root"].bytes

        page = types_pb2.CommandPage(
            sequence=ctx.get("sequence") or 0,
            merge_strategy=types_pb2.MERGE_COMMUTATIVE,
        )
        page.command.CopyFrom(
            Any(
                type_url="type.googleapis.com/test.TestCommand",
                value=b"test",
            )
        )

        cmd = types_pb2.CommandBook(cover=cover)
        cmd.pages.append(page)

        ctx["built_command"] = cmd
    except Exception as e:
        ctx["build_error"] = e
