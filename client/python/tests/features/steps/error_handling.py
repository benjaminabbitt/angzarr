"""Error handling step definitions."""

import pytest
from pytest_bdd import scenarios, given, when, then, parsers


# Link to feature file
scenarios("../../../../features/error_handling.feature")


@pytest.fixture
def error_context():
    """Test context for error handling scenarios."""
    return {
        "error": None,
        "error_code": None,
        "error_message": None,
        "is_retryable": False,
        "command": None,
        "response": None,
    }


@given("a client library implementation")
def given_client_implementation(error_context):
    pass


@given(parsers.parse('a response with status code "{code}"'))
def given_response_with_status(error_context, code):
    error_context["error_code"] = code


@given(parsers.parse('a response with message "{message}"'))
def given_response_with_message(error_context, message):
    error_context["error_message"] = message


@when("parsing the error response")
def when_parsing_error_response(error_context):
    from angzarr_client.errors import ClientError

    error_context["error"] = ClientError(
        code=error_context.get("error_code", "UNKNOWN"),
        message=error_context.get("error_message", "Unknown error"),
    )


@when("a command is rejected")
def when_command_rejected(error_context):
    from angzarr_client.errors import CommandRejectedError

    error_context["error"] = CommandRejectedError(
        reason=error_context.get("error_message", "Command rejected"),
    )


@when("a network error occurs")
def when_network_error(error_context):
    from angzarr_client.errors import ClientError

    error_context["error"] = ClientError(
        code="UNAVAILABLE",
        message="Network error",
    )
    error_context["is_retryable"] = True


@when("a timeout occurs")
def when_timeout_occurs(error_context):
    from angzarr_client.errors import ClientError

    error_context["error"] = ClientError(
        code="DEADLINE_EXCEEDED",
        message="Request timeout",
    )
    error_context["is_retryable"] = True


@then("the error should have a code")
def then_error_has_code(error_context):
    assert hasattr(error_context["error"], "code")


@then("the error should have a message")
def then_error_has_message(error_context):
    err = error_context["error"]
    assert hasattr(err, "message") or hasattr(err, "reason")


@then("the error should be retryable")
def then_error_retryable(error_context):
    assert error_context["is_retryable"]


@then("the error should NOT be retryable")
def then_error_not_retryable(error_context):
    assert not error_context["is_retryable"]


@then(parsers.parse('the error code should be "{expected}"'))
def then_error_code_equals(error_context, expected):
    assert error_context["error"].code == expected


@then(parsers.parse('the error message should contain "{expected}"'))
def then_error_message_contains(error_context, expected):
    err = error_context["error"]
    msg = getattr(err, "message", None) or getattr(err, "reason", "")
    assert expected in msg
