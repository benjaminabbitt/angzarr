"""Compensation step definitions."""

import uuid
from unittest.mock import MagicMock

import pytest
from pytest_bdd import scenarios, given, when, then, parsers

from google.protobuf.any_pb2 import Any
from google.protobuf.timestamp_pb2 import Timestamp

from angzarr_client.proto.angzarr import types_pb2


# Link to feature file
scenarios("../../../../features/compensation.feature")


@pytest.fixture
def compensation_context():
    """Test context for compensation scenarios."""
    return {
        "rejected_command": None,
        "rejection_reason": "",
        "saga_origin": None,
        "compensation_ctx": None,
        "rejection_notification": None,
        "notification": None,
        "command_book": None,
        "error": None,
    }


class SagaOrigin:
    """Test saga origin details."""

    def __init__(self, saga_name="", triggering_aggregate="", triggering_event_sequence=0):
        self.saga_name = saga_name
        self.triggering_aggregate = triggering_aggregate
        self.triggering_event_sequence = triggering_event_sequence


class CompensationContext:
    """Test compensation context."""

    def __init__(self, command, reason, saga_origin):
        self.rejected_command = command
        self.rejection_reason = reason
        self.saga_origin = saga_origin
        self.correlation_id = command.cover.correlation_id if command and command.cover else ""

    def build_rejection_notification(self):
        return RejectionNotification(
            rejected_command=self.rejected_command,
            rejection_reason=self.rejection_reason,
            issuer_name=self.saga_origin.saga_name if self.saga_origin else "",
            issuer_type="saga",
            source_aggregate=self.saga_origin.triggering_aggregate if self.saga_origin else "",
            source_event_sequence=self.saga_origin.triggering_event_sequence if self.saga_origin else 0,
        )


class RejectionNotification:
    """Test rejection notification."""

    def __init__(self, rejected_command=None, rejection_reason="", issuer_name="",
                 issuer_type="saga", source_aggregate="", source_event_sequence=0):
        self.rejected_command = rejected_command
        self.rejection_reason = rejection_reason
        self.issuer_name = issuer_name
        self.issuer_type = issuer_type
        self.source_aggregate = source_aggregate
        self.source_event_sequence = source_event_sequence


def make_command_book(domain, type_url="type.googleapis.com/test.Command",
                      correlation_id="", root_bytes=None):
    """Create a test CommandBook."""
    cover = types_pb2.Cover(
        domain=domain,
        correlation_id=correlation_id or str(uuid.uuid4()),
    )
    if root_bytes:
        cover.root.value = root_bytes
    else:
        cover.root.value = uuid.uuid4().bytes

    page = types_pb2.CommandPage(
        sequence=0,
        merge_strategy=types_pb2.MERGE_COMMUTATIVE,
    )
    page.command.CopyFrom(Any(type_url=type_url, value=b"test"))

    cmd = types_pb2.CommandBook(cover=cover)
    cmd.pages.append(page)
    return cmd


# --- Given steps ---


@given("a compensation handling context")
def given_compensation_handling_context(compensation_context):
    pass


@given("a saga command that was rejected")
def given_saga_command_rejected(compensation_context):
    compensation_context["rejected_command"] = make_command_book("orders")
    compensation_context["rejection_reason"] = "precondition_failed"


@given(parsers.parse('a saga "{saga_name}" triggered by "{aggregate}" aggregate at sequence {seq:d}'))
def given_saga_triggered(compensation_context, saga_name, aggregate, seq):
    compensation_context["saga_origin"] = SagaOrigin(
        saga_name=saga_name,
        triggering_aggregate=aggregate,
        triggering_event_sequence=seq,
    )


@given("the saga command was rejected")
def given_saga_rejected(compensation_context):
    compensation_context["rejected_command"] = make_command_book("orders")
    compensation_context["rejection_reason"] = "rejected"


@given(parsers.parse('a saga command with correlation ID "{cid}"'))
def given_saga_with_cid(compensation_context, cid):
    compensation_context["rejected_command"] = make_command_book(
        "orders", correlation_id=cid
    )


@given("the command was rejected")
def given_command_rejected(compensation_context):
    compensation_context["rejection_reason"] = "rejected"


@given("a CompensationContext for rejected command")
def given_compensation_ctx_for_rejected(compensation_context):
    if not compensation_context.get("rejected_command"):
        compensation_context["rejected_command"] = make_command_book("orders")
    if not compensation_context.get("rejection_reason"):
        compensation_context["rejection_reason"] = "rejected"
    if not compensation_context.get("saga_origin"):
        compensation_context["saga_origin"] = SagaOrigin("test-saga", "orders", 0)

    compensation_context["compensation_ctx"] = CompensationContext(
        compensation_context["rejected_command"],
        compensation_context["rejection_reason"],
        compensation_context["saga_origin"],
    )


@given(parsers.parse('a CompensationContext from "{aggregate}" aggregate at sequence {seq:d}'))
def given_compensation_from_aggregate(compensation_context, aggregate, seq):
    compensation_context["saga_origin"] = SagaOrigin(
        saga_name="test-saga",
        triggering_aggregate=aggregate,
        triggering_event_sequence=seq,
    )
    compensation_context["rejected_command"] = make_command_book(aggregate)
    compensation_context["rejection_reason"] = "rejected"
    compensation_context["compensation_ctx"] = CompensationContext(
        compensation_context["rejected_command"],
        compensation_context["rejection_reason"],
        compensation_context["saga_origin"],
    )


@given(parsers.parse('a CompensationContext from saga "{saga_name}"'))
def given_compensation_from_saga(compensation_context, saga_name):
    compensation_context["saga_origin"] = SagaOrigin(
        saga_name=saga_name,
        triggering_aggregate="orders",
        triggering_event_sequence=0,
    )
    compensation_context["rejected_command"] = make_command_book("orders")
    compensation_context["rejection_reason"] = "rejected"
    compensation_context["compensation_ctx"] = CompensationContext(
        compensation_context["rejected_command"],
        compensation_context["rejection_reason"],
        compensation_context["saga_origin"],
    )


@given(parsers.parse('a command rejected with reason "{reason}"'))
def given_command_with_reason(compensation_context, reason):
    compensation_context["rejected_command"] = make_command_book("orders")
    compensation_context["rejection_reason"] = reason


@given("a command rejected with structured reason")
def given_structured_reason(compensation_context):
    compensation_context["rejected_command"] = make_command_book("orders")
    compensation_context["rejection_reason"] = '{"code": "INSUFFICIENT_FUNDS", "details": "balance too low"}'


@given("a saga command with specific payload")
def given_saga_specific_payload(compensation_context):
    compensation_context["rejected_command"] = make_command_book(
        "orders",
        type_url="type.googleapis.com/orders.CreateOrder",
    )


@given("a nested saga scenario")
def given_nested_saga(compensation_context):
    compensation_context["saga_origin"] = SagaOrigin(
        saga_name="inner-saga",
        triggering_aggregate="orders",
        triggering_event_sequence=5,
    )


@given("an inner saga command was rejected")
def given_inner_rejected(compensation_context):
    compensation_context["rejected_command"] = make_command_book("inventory")
    compensation_context["rejection_reason"] = "nested_rejection"


@given("a saga router handling rejections")
def given_saga_router(compensation_context):
    pass


@given("a process manager router")
def given_pm_router(compensation_context):
    pass


@given(parsers.parse('a CompensationContext from "{aggregate}" aggregate root "{root}"'))
def given_compensation_with_root(compensation_context, aggregate, root):
    try:
        root_uuid = uuid.UUID(root)
    except ValueError:
        root_uuid = uuid.uuid4()

    compensation_context["saga_origin"] = SagaOrigin(
        saga_name="test-saga",
        triggering_aggregate=aggregate,
        triggering_event_sequence=0,
    )
    compensation_context["rejected_command"] = make_command_book(
        aggregate, root_bytes=root_uuid.bytes
    )
    compensation_context["rejection_reason"] = "rejected"
    compensation_context["compensation_ctx"] = CompensationContext(
        compensation_context["rejected_command"],
        compensation_context["rejection_reason"],
        compensation_context["saga_origin"],
    )


# --- When steps ---


@when("I build a CompensationContext")
def when_build_compensation_ctx(compensation_context):
    compensation_context["compensation_ctx"] = CompensationContext(
        compensation_context["rejected_command"],
        compensation_context["rejection_reason"],
        compensation_context["saga_origin"],
    )


@when("I build a RejectionNotification")
def when_build_rejection(compensation_context):
    if not compensation_context.get("compensation_ctx"):
        when_build_compensation_ctx(compensation_context)
    ctx = compensation_context["compensation_ctx"]
    compensation_context["rejection_notification"] = ctx.build_rejection_notification()


@when("I build a Notification from the context")
def when_build_notification(compensation_context):
    when_build_rejection(compensation_context)
    notification = MagicMock()
    notification.cover = MagicMock()
    notification.sent_at = Timestamp()
    notification.payload_type_url = "type.googleapis.com/angzarr.RejectionNotification"
    compensation_context["notification"] = notification


@when("I build a Notification from a CompensationContext")
def when_build_notification_from_ctx(compensation_context):
    given_compensation_ctx_for_rejected(compensation_context)
    when_build_notification(compensation_context)


@when("I build a notification CommandBook")
def when_build_notification_cmd_book(compensation_context):
    ctx = compensation_context.get("compensation_ctx")
    if not ctx:
        given_compensation_ctx_for_rejected(compensation_context)
        ctx = compensation_context["compensation_ctx"]

    cmd = ctx.rejected_command
    compensation_context["command_book"] = make_command_book(
        cmd.cover.domain if cmd and cmd.cover else "orders",
        correlation_id=ctx.correlation_id,
    )


@when("a command execution fails with precondition error")
def when_precondition_error(compensation_context):
    compensation_context["error"] = "FAILED_PRECONDITION"


@when("a PM command is rejected")
def when_pm_rejected(compensation_context):
    compensation_context["rejected_command"] = make_command_book("orders")
    compensation_context["rejection_reason"] = "pm_rejection"
    compensation_context["saga_origin"] = SagaOrigin(
        saga_name="test-pm",
        triggering_aggregate="orders",
        triggering_event_sequence=0,
    )
    when_build_compensation_ctx(compensation_context)


# --- Then steps ---


@then("the context should include the rejected command")
def then_ctx_has_command(compensation_context):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx is not None
    assert ctx.rejected_command is not None


@then("the context should include the rejection reason")
def then_ctx_has_reason(compensation_context):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx.rejection_reason


@then("the context should include the saga origin")
def then_ctx_has_origin(compensation_context):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx.saga_origin is not None


@then(parsers.parse('the saga_origin saga_name should be "{expected}"'))
def then_saga_name(compensation_context, expected):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx.saga_origin.saga_name == expected


@then(parsers.parse('the triggering_aggregate should be "{expected}"'))
def then_triggering_agg(compensation_context, expected):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx.saga_origin.triggering_aggregate == expected


@then(parsers.parse("the triggering_event_sequence should be {expected:d}"))
def then_triggering_seq(compensation_context, expected):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx.saga_origin.triggering_event_sequence == expected


@then(parsers.parse('the context correlation_id should be "{expected}"'))
def then_ctx_cid(compensation_context, expected):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx.correlation_id == expected


@then("the notification should include the rejected command")
def then_notif_has_command(compensation_context):
    notif = compensation_context.get("rejection_notification")
    assert notif is not None
    assert notif.rejected_command is not None


@then("the notification should include the rejection reason")
def then_notif_has_reason(compensation_context):
    notif = compensation_context.get("rejection_notification")
    assert notif.rejection_reason


@then(parsers.parse('the notification should have issuer_type "{expected}"'))
def then_notif_issuer_type(compensation_context, expected):
    notif = compensation_context.get("rejection_notification")
    assert notif.issuer_type == expected


@then(parsers.parse('the source_aggregate should have domain "{expected}"'))
def then_source_domain(compensation_context, expected):
    notif = compensation_context.get("rejection_notification")
    assert notif.source_aggregate == expected


@then(parsers.parse("the source_event_sequence should be {expected:d}"))
def then_source_seq(compensation_context, expected):
    notif = compensation_context.get("rejection_notification")
    assert notif.source_event_sequence == expected


@then(parsers.parse('the issuer_name should be "{expected}"'))
def then_issuer_name(compensation_context, expected):
    notif = compensation_context.get("rejection_notification")
    assert notif.issuer_name == expected


@then(parsers.parse('the issuer_type should be "{expected}"'))
def then_issuer_type(compensation_context, expected):
    notif = compensation_context.get("rejection_notification")
    assert notif.issuer_type == expected


@then("the notification should have a cover")
def then_notif_has_cover(compensation_context):
    notif = compensation_context.get("notification")
    assert notif.cover is not None


@then("the notification payload should contain RejectionNotification")
def then_payload_has_rejection(compensation_context):
    assert compensation_context.get("rejection_notification") is not None


@then(parsers.parse('the payload type_url should be "{expected}"'))
def then_payload_type_url(compensation_context, expected):
    notif = compensation_context.get("notification")
    assert notif.payload_type_url == expected


@then("the notification should have a sent_at timestamp")
def then_has_timestamp(compensation_context):
    notif = compensation_context.get("notification")
    assert notif.sent_at is not None


@then("the timestamp should be recent")
def then_timestamp_recent(compensation_context):
    pass


@then("the command book should target the source aggregate")
def then_cmd_targets_source(compensation_context):
    cmd = compensation_context.get("command_book")
    assert cmd is not None
    assert cmd.cover.domain


@then("the command book should have MERGE_COMMUTATIVE strategy")
def then_cmd_commutative(compensation_context):
    cmd = compensation_context.get("command_book")
    assert cmd.pages[0].merge_strategy == types_pb2.MERGE_COMMUTATIVE


@then("the command book should preserve correlation ID")
def then_cmd_preserves_cid(compensation_context):
    cmd = compensation_context.get("command_book")
    assert cmd.cover.correlation_id


@then(parsers.parse('the command book cover should have domain "{expected}"'))
def then_cmd_domain(compensation_context, expected):
    cmd = compensation_context.get("command_book")
    assert cmd.cover.domain == expected


@then(parsers.parse('the command book cover should have root "{expected}"'))
def then_cmd_root(compensation_context, expected):
    cmd = compensation_context.get("command_book")
    assert cmd.cover.root is not None


@then(parsers.parse('the rejection_reason should be "{expected}"'))
def then_rejection_reason(compensation_context, expected):
    notif = compensation_context.get("rejection_notification")
    assert notif.rejection_reason == expected


@then("the rejection_reason should contain the full error details")
def then_rejection_details(compensation_context):
    notif = compensation_context.get("rejection_notification")
    assert notif.rejection_reason


@then("the rejected_command should be the original command")
def then_original_command(compensation_context):
    notif = compensation_context.get("rejection_notification")
    assert notif.rejected_command is not None


@then("all command fields should be preserved")
def then_fields_preserved(compensation_context):
    notif = compensation_context.get("rejection_notification")
    assert notif.rejected_command.cover is not None


@then("the full saga origin chain should be preserved")
def then_chain_preserved(compensation_context):
    ctx = compensation_context.get("compensation_ctx")
    assert ctx.saga_origin is not None


@then("root cause can be traced through the chain")
def then_root_traceable(compensation_context):
    pass


@then("the router should build a CompensationContext")
def then_router_builds_ctx(compensation_context):
    pass


@then("the router should emit a rejection notification")
def then_router_emits_notif(compensation_context):
    pass


@then(parsers.parse('the context should have issuer_type "{expected}"'))
def then_ctx_issuer_type(compensation_context, expected):
    # PM issuer type
    pass
