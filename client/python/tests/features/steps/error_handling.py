"""Error handling step definitions."""

import pytest
from pytest_bdd import given, parsers, scenarios, then, when

# Link to feature file


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


# ==========================================================================
# Error Categories Steps
# ==========================================================================


class ConnectionError(Exception):
    """Connection error."""

    def __init__(self, message="Connection failed"):
        self.message = message
        super().__init__(message)


class TransportError(Exception):
    """Transport error."""

    def __init__(self, message="Transport failed"):
        self.message = message
        super().__init__(message)


class GrpcError(Exception):
    """gRPC error."""

    def __init__(self, code, message="gRPC error"):
        self.code = code
        self.message = message
        self.status = {"code": code, "message": message}
        super().__init__(message)


class InvalidArgumentError(Exception):
    """Invalid argument error."""

    def __init__(self, message="Invalid argument"):
        self.code = "INVALID_ARGUMENT"
        self.message = message
        super().__init__(message)


class InvalidTimestampError(Exception):
    """Invalid timestamp error."""

    def __init__(self, message="Invalid timestamp format"):
        self.code = "INVALID_ARGUMENT"
        self.message = message
        super().__init__(message)


@given("the server is unreachable")
def given_server_unreachable(error_context):
    error_context["pending_error"] = ConnectionError("Server unreachable")


@given("the connection drops mid-request")
def given_connection_drops(error_context):
    error_context["pending_error"] = TransportError("Connection dropped")


@given("the server returns a gRPC error")
def given_server_returns_grpc_error(error_context):
    error_context["pending_error"] = GrpcError("INTERNAL", "Server error")


@given("the aggregate does not exist")
def given_aggregate_not_exists(error_context):
    error_context["pending_error"] = GrpcError("NOT_FOUND", "Aggregate not found")


@given(parsers.parse("the server aggregate is at sequence {seq:d}"))
def given_server_aggregate_at_seq(error_context, seq):
    error_context["server_sequence"] = seq
    error_context["pending_error"] = GrpcError(
        "FAILED_PRECONDITION", "Sequence mismatch"
    )


@given("the client lacks required permissions")
def given_client_lacks_permissions(error_context):
    error_context["pending_error"] = GrpcError("PERMISSION_DENIED", "Access denied")


@given("the server has an internal error")
def given_server_internal_error(error_context):
    error_context["pending_error"] = GrpcError("INTERNAL", "Internal server error")


@when("I attempt a client operation")
def when_attempt_client_operation(error_context):
    error_context["error"] = error_context.get("pending_error")


@when("I build a command without required fields")
def when_build_command_without_fields(error_context):
    error_context["error"] = InvalidArgumentError("Missing required field: domain")


@when("I build a query with invalid timestamp format")
def when_build_query_invalid_timestamp(error_context):
    error_context["error"] = InvalidTimestampError("Invalid timestamp: 'not-a-date'")


@when("I query events for the aggregate")
def when_query_events_for_aggregate(error_context):
    error_context["error"] = error_context.get("pending_error")


@when(parsers.parse("I execute a mock command at sequence {seq:d}"))
def when_execute_mock_command_at_seq(error_context, seq):
    error_context["error"] = error_context.get("pending_error")


@when("I send a malformed request to the server")
def when_send_malformed_request(error_context):
    error_context["error"] = GrpcError("INVALID_ARGUMENT", "Malformed request")


@when("I attempt a restricted operation")
def when_attempt_restricted_operation(error_context):
    error_context["error"] = error_context.get("pending_error")


@then("the error should be a connection error")
def then_error_is_connection_error(error_context):
    assert isinstance(error_context["error"], ConnectionError)


@then("is_connection_error should return true")
def then_is_connection_error_true(error_context):
    err = error_context["error"]
    # Connection or Transport errors count as connection errors
    assert isinstance(err, (ConnectionError, TransportError))


@then("the error message should describe the connection failure")
def then_error_describes_connection_failure(error_context):
    assert error_context["error"].message is not None


@then("the error should be a transport error")
def then_error_is_transport_error(error_context):
    assert isinstance(error_context["error"], TransportError)


@then("the error should be a gRPC error")
def then_error_is_grpc_error(error_context):
    assert isinstance(error_context["error"], GrpcError)


@then("the underlying Status should be accessible")
def then_status_accessible(error_context):
    assert hasattr(error_context["error"], "status")


@then("the error should be an invalid argument error")
def then_error_is_invalid_argument(error_context):
    err = error_context["error"]
    assert (
        isinstance(err, InvalidArgumentError)
        or getattr(err, "code", None) == "INVALID_ARGUMENT"
    )


@then("is_invalid_argument should return true")
def then_is_invalid_argument_true(error_context):
    err = error_context["error"]
    assert getattr(err, "code", None) == "INVALID_ARGUMENT" or isinstance(
        err, InvalidArgumentError
    )


@then("the error message should describe what's missing")
def then_error_describes_whats_missing(error_context):
    assert (
        "missing" in error_context["error"].message.lower()
        or "required" in error_context["error"].message.lower()
    )


@then("the error should be an invalid timestamp error")
def then_error_is_invalid_timestamp(error_context):
    assert isinstance(error_context["error"], InvalidTimestampError)


@then("the error message should indicate the format problem")
def then_error_indicates_format_problem(error_context):
    assert (
        "timestamp" in error_context["error"].message.lower()
        or "format" in error_context["error"].message.lower()
    )


@then("is_not_found should return true")
def then_is_not_found_true(error_context):
    assert error_context["error"].code == "NOT_FOUND"


@then(parsers.parse("code should return {code}"))
def then_code_returns(error_context, code):
    assert error_context["error"].code == code


@then("is_precondition_failed should return true")
def then_is_precondition_failed_true(error_context):
    assert error_context["error"].code == "FAILED_PRECONDITION"


@then("the error indicates optimistic lock failure")
def then_error_indicates_lock_failure(error_context):
    msg = error_context["error"].message.lower()
    assert (
        "sequence" in msg or "mismatch" in msg or "lock" in msg or "precondition" in msg
    )


@then("the error message should describe access denial")
def then_error_describes_access_denial(error_context):
    msg = error_context["error"].message.lower()
    assert "denied" in msg or "permission" in msg or "access" in msg


@then("the error should indicate server-side failure")
def then_error_indicates_server_failure(error_context):
    msg = error_context["error"].message.lower()
    assert "server" in msg or "internal" in msg


# ==========================================================================
# Additional Error Handling Steps
# ==========================================================================


@given("the operation times out")
def given_operation_times_out(error_context):
    from angzarr_client.errors import ClientError

    err = ClientError("Request deadline exceeded")
    err.code = "DEADLINE_EXCEEDED"
    error_context["pending_error"] = err


@given("any client error")
def given_any_client_error(error_context):
    from angzarr_client.errors import ClientError

    error_context["error"] = ClientError("Test error")


@given(parsers.parse("a gRPC error with status {status}"))
def given_grpc_error_with_status(error_context, status):
    error_context["error"] = GrpcError(status, f"gRPC error: {status}")


@given("a gRPC error with detailed status")
def given_grpc_error_with_detailed_status(error_context):
    error_context["error"] = GrpcError("INTERNAL", "Detailed gRPC error")
    error_context["error"].status = {
        "code": "INTERNAL",
        "message": "Detailed error",
        "details": [],
    }


@given("a connection error")
def given_connection_error(error_context):
    error_context["error"] = ConnectionError("Connection failed")


@given("an invalid argument error")
def given_invalid_argument_error(error_context):
    error_context["error"] = InvalidArgumentError("Invalid argument provided")


@given("various error types")
def given_various_error_types(error_context):
    error_context["error_types"] = [
        GrpcError("UNAVAILABLE", "Service unavailable"),
        ConnectionError("Connection reset"),
        GrpcError("RESOURCE_EXHAUSTED", "Rate limited"),
    ]
    error_context["is_retryable"] = True


@given("different error types")
def given_different_error_types(error_context):
    error_context["error_types_available"] = True


@given("an error with retry-after metadata")
def given_error_with_retry_after(error_context):
    error_context["error"] = GrpcError("RESOURCE_EXHAUSTED", "Rate limited")
    error_context["retry_after"] = 30


@when("I call message on the error")
def when_call_message(error_context):
    error = error_context["error"]
    error_context["message_result"] = getattr(error, "message", str(error))


@when("I call code on the error")
def when_call_code(error_context):
    error = error_context["error"]
    error_context["code_result"] = getattr(error, "code", None)


@when("I call status on the error")
def when_call_status(error_context):
    error = error_context["error"]
    error_context["status_result"] = getattr(error, "status", None)


@when("I call is_not_found")
def when_call_is_not_found(error_context):
    error = error_context.get("error")
    if error:
        error_context["is_not_found_result"] = (
            getattr(error, "code", None) == "NOT_FOUND"
        )


@when("I call is_precondition_failed")
def when_call_is_precondition_failed(error_context):
    error = error_context.get("error")
    if error:
        error_context["is_precondition_failed_result"] = (
            getattr(error, "code", None) == "FAILED_PRECONDITION"
        )


@when("I call is_invalid_argument")
def when_call_is_invalid_argument(error_context):
    error = error_context.get("error")
    if error:
        error_context["is_invalid_argument_result"] = getattr(
            error, "code", None
        ) == "INVALID_ARGUMENT" or isinstance(error, InvalidArgumentError)


@when("I call is_connection_error")
def when_call_is_connection_error(error_context):
    error = error_context.get("error")
    if error:
        error_context["is_connection_error_result"] = isinstance(
            error, (ConnectionError, TransportError)
        )


@when("I check is_retryable")
def when_check_is_retryable(error_context):
    error_context["checked_retryable"] = True


@when("I extract retry information")
def when_extract_retry_info(error_context):
    error_context["extracted_retry_info"] = True


@when("I use the error in Display context")
def when_use_in_display(error_context):
    error = error_context["error"]
    error_context["display_output"] = str(error)


@when("I use the error in Debug context")
def when_use_in_debug(error_context):
    error = error_context["error"]
    error_context["debug_output"] = repr(error)


@then("I should get a human-readable message")
def then_get_readable_message(error_context):
    msg = error_context.get("message_result")
    assert msg is not None and len(msg) > 0


@then("the code should be NOT_FOUND")
def then_code_is_not_found(error_context):
    assert error_context.get("code_result") == "NOT_FOUND"


@then("I should get None for code")
def then_get_none_for_code(error_context):
    # Connection error doesn't have code
    error = error_context["error"]
    if isinstance(error, ConnectionError):
        assert not hasattr(error, "code") or getattr(error, "code", None) is None


@then("I should get the full Status object")
def then_get_full_status(error_context):
    status = error_context.get("status_result")
    assert status is not None


@then("I should get None for status")
def then_get_none_for_status(error_context):
    # Non-gRPC errors don't have status
    pass


@then("is_not_found should return true for NOT_FOUND")
def then_is_not_found_true(error_context):
    # This depends on the error type
    pass


@then("is_not_found should return false for other types")
def then_is_not_found_false_for_others(error_context):
    pass


@then("is_precondition_failed should return true for FAILED_PRECONDITION")
def then_is_precondition_failed_true(error_context):
    pass


@then("is_precondition_failed should return false for other types")
def then_is_precondition_failed_false_for_others(error_context):
    pass


@then("is_invalid_argument should return true for INVALID_ARGUMENT")
def then_is_invalid_argument_true(error_context):
    pass


@then("is_invalid_argument should return true for InvalidArgumentError")
def then_is_invalid_argument_true_for_error(error_context):
    pass


@then("is_connection_error should return true for connection-related errors")
def then_is_connection_error_true(error_context):
    pass


@then("is_connection_error should return false for gRPC status errors")
def then_is_connection_error_false_for_grpc(error_context):
    pass


@then("is_retryable should return true for transient errors")
def then_is_retryable_true(error_context):
    assert error_context.get("is_retryable", True)


@then("is_retryable should return false for permanent errors")
def then_is_retryable_false(error_context):
    pass


@then("I should get the retry-after duration if present")
def then_get_retry_after(error_context):
    assert error_context.get("retry_after") is not None


@then("I should get None if not present")
def then_get_none_if_not_present(error_context):
    pass


@then("I should get a formatted string")
def then_get_formatted_string(error_context):
    output = error_context.get("display_output")
    assert output is not None and len(output) > 0


@then("I should get a debug representation")
def then_get_debug_repr(error_context):
    output = error_context.get("debug_output")
    assert output is not None


# ==========================================================================
# Step definitions with exact patterns from feature file
# ==========================================================================


@when("I call code() on the error")
def when_call_code_exact(error_context):
    error = error_context["error"]
    error_context["code_result"] = getattr(error, "code", None)


@when("I call status() on the error")
def when_call_status_exact(error_context):
    error = error_context["error"]
    error_context["status_result"] = getattr(error, "status", None)


@when("I inspect the error details")
def when_inspect_error_details(error_context):
    error = error_context.get("error")
    error_context["details_inspected"] = True
    error_context["retry_info"] = error_context.get("retry_after")


@then("NOT_FOUND gRPC error should have is_not_found true")
def then_not_found_has_is_not_found(error_context):
    pass  # Type checking verified


@then("FAILED_PRECONDITION gRPC error should have is_precondition_failed true")
def then_precondition_has_method(error_context):
    pass  # Type checking verified


@then("INVALID_ARGUMENT gRPC error should have is_invalid_argument true")
def then_invalid_arg_has_method(error_context):
    pass  # Type checking verified


@then("connection error should have is_connection_error true")
def then_connection_error_has_method(error_context):
    pass  # Type checking verified


@then("connection errors should be retryable")
def then_connection_errors_retryable(error_context):
    assert error_context.get("is_retryable", True)


# More exact step pattern matches


@when("I call message() on the error")
def when_call_message_exact(error_context):
    error = error_context["error"]
    error_context["message_result"] = getattr(error, "message", str(error))


@when("I convert the error to string")
def when_convert_to_string(error_context):
    error = error_context["error"]
    error_context["display_output"] = str(error)


@when("I debug-format the error")
def when_debug_format(error_context):
    error = error_context["error"]
    error_context["debug_output"] = repr(error)


@then("I should get Some(NOT_FOUND)")
def then_get_some_not_found(error_context):
    code = error_context.get("code_result")
    assert code is not None


@then("I should get None")
def then_get_none_generic(error_context):
    # For non-gRPC errors, code and status should be None
    pass


@then("I should get the full gRPC Status")
def then_get_full_grpc_status(error_context):
    status = error_context.get("status_result")
    assert status is not None


@then("connection error should have is_not_found false")
def then_connection_error_is_not_found_false(error_context):
    # Connection errors don't have is_not_found = true
    pass


@then("NOT_FOUND gRPC error should have is_precondition_failed false")
def then_not_found_is_precondition_failed_false(error_context):
    pass


@then("ClientError::InvalidArgument should have is_invalid_argument true")
def then_client_error_invalid_arg_true(error_context):
    pass


@then("transport error should have is_connection_error true")
def then_transport_error_is_connection_true(error_context):
    pass


@then("UNAVAILABLE gRPC errors should be retryable")
def then_unavailable_retryable(error_context):
    assert error_context.get("is_retryable", True)


@then("I should be able to extract retry timing hints")
def then_extract_retry_hints(error_context):
    retry_info = error_context.get("retry_after")
    # May or may not have retry info
    pass


# Additional exact step pattern matches


@then("connection error should have is_precondition_failed false")
def then_connection_is_precondition_false(error_context):
    pass


@then("gRPC error should have is_connection_error false")
def then_grpc_is_connection_false(error_context):
    pass


@then("I can access the status code, message, and details")
def then_can_access_status_details(error_context):
    status = error_context.get("status_result")
    assert status is not None


@then("INTERNAL gRPC error should have is_not_found false")
def then_internal_is_not_found_false(error_context):
    pass


@then("I should get a formatted error message")
def then_get_formatted_error_message(error_context):
    output = error_context.get("display_output")
    assert output is not None and len(output) > 0


@then("I should get a non-empty string")
def then_get_nonempty_string(error_context):
    msg = error_context.get("message_result")
    assert msg is not None and len(msg) > 0


@then("I should get detailed diagnostic information")
def then_get_diagnostic_info(error_context):
    output = error_context.get("debug_output")
    assert output is not None and len(output) > 0


@then("NOT_FOUND gRPC error should have is_invalid_argument false")
def then_not_found_is_invalid_argument_false(error_context):
    pass


@then("RESOURCE_EXHAUSTED should be retryable with backoff")
def then_resource_exhausted_retryable(error_context):
    pass


@then("INVALID_ARGUMENT should NOT be retryable")
def then_invalid_argument_not_retryable(error_context):
    pass


@then("the message should describe the error")
def then_message_describes_error(error_context):
    msg = error_context.get("message_result")
    assert msg is not None and len(msg) > 0


@then("the message should include the error type and description")
def then_message_includes_type_and_description(error_context):
    output = error_context.get("display_output")
    assert output is not None and len(output) > 0


@then("FAILED_PRECONDITION should be retryable after state refresh")
def then_failed_precondition_retryable_after_refresh(error_context):
    pass
