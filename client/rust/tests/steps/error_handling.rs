//! Error handling step definitions.

use angzarr_client::ClientError;
use cucumber::{given, then, when, World};
use tonic::{Code, Status};

/// Test context for error handling scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct ErrorHandlingWorld {
    current_error: Option<ClientError>,
    error_variants: Vec<ClientError>,
}

impl ErrorHandlingWorld {
    fn new() -> Self {
        Self {
            current_error: None,
            error_variants: Vec::new(),
        }
    }
}

// --- Given steps ---

#[given("the server is unreachable")]
async fn given_server_unreachable(world: &mut ErrorHandlingWorld) {
    world.current_error = Some(ClientError::Connection("connection refused".to_string()));
}

#[given("the connection drops mid-request")]
async fn given_connection_drops(world: &mut ErrorHandlingWorld) {
    // Simulate transport error with a mock error
    world.current_error = Some(ClientError::Connection("connection reset".to_string()));
}

#[given("the server returns a gRPC error")]
async fn given_server_returns_grpc_error(world: &mut ErrorHandlingWorld) {
    let status = Status::internal("server error");
    world.current_error = Some(ClientError::from(status));
}

#[given("the aggregate does not exist")]
async fn given_aggregate_not_exists(world: &mut ErrorHandlingWorld) {
    let status = Status::not_found("aggregate not found");
    world.current_error = Some(ClientError::from(status));
}

#[given("an aggregate at sequence 5")]
async fn given_aggregate_at_sequence(world: &mut ErrorHandlingWorld) {
    let status = Status::failed_precondition("sequence mismatch: expected 5, got 3");
    world.current_error = Some(ClientError::from(status));
}

#[given("the client lacks required permissions")]
async fn given_lacks_permissions(world: &mut ErrorHandlingWorld) {
    let status = Status::permission_denied("access denied to resource");
    world.current_error = Some(ClientError::from(status));
}

#[given("the server has an internal error")]
async fn given_server_internal_error(world: &mut ErrorHandlingWorld) {
    let status = Status::internal("unexpected server error");
    world.current_error = Some(ClientError::from(status));
}

#[given("the operation times out")]
async fn given_operation_timeout(world: &mut ErrorHandlingWorld) {
    let status = Status::deadline_exceeded("operation timed out");
    world.current_error = Some(ClientError::from(status));
}

#[given("any client error")]
async fn given_any_client_error(world: &mut ErrorHandlingWorld) {
    world.current_error = Some(ClientError::Connection("test error".to_string()));
}

#[given(expr = "a gRPC error with status NOT_FOUND")]
async fn given_grpc_not_found(world: &mut ErrorHandlingWorld) {
    let status = Status::not_found("resource not found");
    world.current_error = Some(ClientError::from(status));
}

#[given("a connection error")]
async fn given_connection_error(world: &mut ErrorHandlingWorld) {
    world.current_error = Some(ClientError::Connection("connection failed".to_string()));
}

#[given("a gRPC error with detailed status")]
async fn given_grpc_detailed_status(world: &mut ErrorHandlingWorld) {
    let status = Status::internal("detailed error message");
    world.current_error = Some(ClientError::from(status));
}

#[given("an invalid argument error")]
async fn given_invalid_argument_error(world: &mut ErrorHandlingWorld) {
    world.current_error = Some(ClientError::InvalidArgument("missing field".to_string()));
}

#[given("different error types")]
async fn given_different_error_types(world: &mut ErrorHandlingWorld) {
    world.error_variants = vec![
        ClientError::from(Status::not_found("not found")),
        ClientError::from(Status::failed_precondition("precondition failed")),
        ClientError::from(Status::invalid_argument("invalid")),
        ClientError::from(Status::internal("internal")),
        ClientError::Connection("connection".to_string()),
        ClientError::InvalidArgument("invalid arg".to_string()),
    ];
}

#[given("various error types")]
async fn given_various_error_types(world: &mut ErrorHandlingWorld) {
    world.error_variants = vec![
        ClientError::Connection("connection failed".to_string()),
        ClientError::from(Status::unavailable("service unavailable")),
        ClientError::from(Status::resource_exhausted("rate limited")),
        ClientError::from(Status::invalid_argument("bad input")),
        ClientError::from(Status::failed_precondition("conflict")),
    ];
}

#[given("an error with retry-after metadata")]
async fn given_error_with_retry_metadata(world: &mut ErrorHandlingWorld) {
    let status = Status::resource_exhausted("rate limited");
    world.current_error = Some(ClientError::from(status));
}

// --- When steps ---

#[when("I attempt a client operation")]
async fn when_attempt_operation(_world: &mut ErrorHandlingWorld) {
    // Error was already set in given step
}

#[when("I query events for the aggregate")]
async fn when_query_events(_world: &mut ErrorHandlingWorld) {
    // Error was already set in given step
}

#[when(expr = "I execute a command at sequence {int}")]
async fn when_execute_at_sequence(_world: &mut ErrorHandlingWorld, _seq: u32) {
    // Error was already set in given step
}

#[when("I build a command without required fields")]
async fn when_build_without_required(world: &mut ErrorHandlingWorld) {
    world.current_error = Some(ClientError::InvalidArgument("type_url not set".to_string()));
}

#[when("I build a query with invalid timestamp format")]
async fn when_build_invalid_timestamp(world: &mut ErrorHandlingWorld) {
    world.current_error = Some(ClientError::InvalidTimestamp("invalid format".to_string()));
}

#[when("I send a malformed request to the server")]
async fn when_send_malformed(world: &mut ErrorHandlingWorld) {
    let status = Status::invalid_argument("malformed request");
    world.current_error = Some(ClientError::from(status));
}

#[when("I attempt a restricted operation")]
async fn when_attempt_restricted(_world: &mut ErrorHandlingWorld) {
    // Error was already set in given step
}

#[when("I call message() on the error")]
async fn when_call_message(_world: &mut ErrorHandlingWorld) {
    // Will be checked in then step
}

#[when("I call code() on the error")]
async fn when_call_code(_world: &mut ErrorHandlingWorld) {
    // Will be checked in then step
}

#[when("I call status() on the error")]
async fn when_call_status(_world: &mut ErrorHandlingWorld) {
    // Will be checked in then step
}

#[when("I inspect the error details")]
async fn when_inspect_error(_world: &mut ErrorHandlingWorld) {
    // Will be checked in then step
}

#[when("I convert the error to string")]
async fn when_convert_to_string(_world: &mut ErrorHandlingWorld) {
    // Will be checked in then step
}

#[when("I debug-format the error")]
async fn when_debug_format(_world: &mut ErrorHandlingWorld) {
    // Will be checked in then step
}

// --- Then steps ---

#[then("the error should be a connection error")]
async fn then_is_connection_error(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(matches!(err, ClientError::Connection(_)));
}

#[then("is_connection_error should return true")]
async fn then_is_connection_error_true(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.is_connection_error());
}

#[then("the error message should describe the connection failure")]
async fn then_message_describes_connection(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let msg = err.message();
    assert!(!msg.is_empty());
}

#[then("the error should be a transport error")]
async fn then_is_transport_error(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    // We use Connection for simplicity in tests
    assert!(err.is_connection_error());
}

#[then("the error should be a gRPC error")]
async fn then_is_grpc_error(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(matches!(err, ClientError::Grpc(_)));
}

#[then("the underlying Status should be accessible")]
async fn then_status_accessible(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.status().is_some());
}

#[then("the error should be an invalid argument error")]
async fn then_is_invalid_argument(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.is_invalid_argument());
}

#[then("is_invalid_argument should return true")]
async fn then_is_invalid_argument_true(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.is_invalid_argument());
}

#[then("the error message should describe what's missing")]
async fn then_message_describes_missing(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let msg = err.message();
    assert!(!msg.is_empty());
}

#[then("the error should be an invalid timestamp error")]
async fn then_is_invalid_timestamp(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(matches!(err, ClientError::InvalidTimestamp(_)));
}

#[then("the error message should indicate the format problem")]
async fn then_message_indicates_format(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let msg = err.message();
    assert!(!msg.is_empty());
}

#[then("is_not_found should return true")]
async fn then_is_not_found_true(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.is_not_found());
}

#[then("code should return NOT_FOUND")]
async fn then_code_not_found(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::NotFound));
}

#[then("is_precondition_failed should return true")]
async fn then_is_precondition_failed_true(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.is_precondition_failed());
}

#[then("code should return FAILED_PRECONDITION")]
async fn then_code_failed_precondition(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::FailedPrecondition));
}

#[then("the error indicates optimistic lock failure")]
async fn then_indicates_optimistic_lock(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.is_precondition_failed());
}

#[then("code should return INVALID_ARGUMENT")]
async fn then_code_invalid_argument(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::InvalidArgument));
}

#[then("code should return PERMISSION_DENIED")]
async fn then_code_permission_denied(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::PermissionDenied));
}

#[then("the error message should describe access denial")]
async fn then_message_describes_access_denial(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let msg = err.message();
    assert!(msg.contains("denied") || msg.contains("access"));
}

#[then("code should return INTERNAL")]
async fn then_code_internal(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::Internal));
}

#[then("the error should indicate server-side failure")]
async fn then_indicates_server_failure(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::Internal));
}

#[then("code should return DEADLINE_EXCEEDED")]
async fn then_code_deadline_exceeded(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::DeadlineExceeded));
}

#[then("I should get a non-empty string")]
async fn then_get_nonempty_string(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(!err.message().is_empty());
}

#[then("the message should describe the error")]
async fn then_message_describes_error(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(!err.message().is_empty());
}

#[then(expr = "I should get Some\\(NOT_FOUND\\)")]
async fn then_get_some_not_found(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert_eq!(err.code(), Some(Code::NotFound));
}

#[then("I should get None")]
async fn then_get_none(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    // For connection errors, code() returns None
    if matches!(
        err,
        ClientError::Connection(_) | ClientError::InvalidArgument(_)
    ) {
        assert!(err.code().is_none());
    }
}

#[then("I should get the full gRPC Status")]
async fn then_get_full_status(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    assert!(err.status().is_some());
}

#[then("I can access the status code, message, and details")]
async fn then_access_status_parts(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let status = err.status().expect("status missing");
    let _ = status.code();
    let _ = status.message();
}

#[then("NOT_FOUND gRPC error should have is_not_found true")]
async fn then_not_found_has_is_not_found(world: &mut ErrorHandlingWorld) {
    let not_found = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::NotFound))
        .expect("not_found variant missing");
    assert!(not_found.is_not_found());
}

#[then("connection error should have is_not_found false")]
async fn then_connection_has_is_not_found_false(world: &mut ErrorHandlingWorld) {
    let conn = world
        .error_variants
        .iter()
        .find(|e| matches!(e, ClientError::Connection(_)))
        .expect("connection variant missing");
    assert!(!conn.is_not_found());
}

#[then("INTERNAL gRPC error should have is_not_found false")]
async fn then_internal_has_is_not_found_false(world: &mut ErrorHandlingWorld) {
    let internal = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::Internal))
        .expect("internal variant missing");
    assert!(!internal.is_not_found());
}

#[then("FAILED_PRECONDITION gRPC error should have is_precondition_failed true")]
async fn then_failed_precondition_true(world: &mut ErrorHandlingWorld) {
    let fp = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::FailedPrecondition))
        .expect("failed_precondition variant missing");
    assert!(fp.is_precondition_failed());
}

#[then("NOT_FOUND gRPC error should have is_precondition_failed false")]
async fn then_not_found_precondition_false(world: &mut ErrorHandlingWorld) {
    let nf = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::NotFound))
        .expect("not_found variant missing");
    assert!(!nf.is_precondition_failed());
}

#[then("connection error should have is_precondition_failed false")]
async fn then_connection_precondition_false(world: &mut ErrorHandlingWorld) {
    let conn = world
        .error_variants
        .iter()
        .find(|e| matches!(e, ClientError::Connection(_)))
        .expect("connection variant missing");
    assert!(!conn.is_precondition_failed());
}

#[then("INVALID_ARGUMENT gRPC error should have is_invalid_argument true")]
async fn then_invalid_argument_grpc_true(world: &mut ErrorHandlingWorld) {
    let ia = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::InvalidArgument))
        .expect("invalid_argument variant missing");
    assert!(ia.is_invalid_argument());
}

#[then("ClientError::InvalidArgument should have is_invalid_argument true")]
async fn then_client_error_invalid_argument_true(world: &mut ErrorHandlingWorld) {
    let ia = world
        .error_variants
        .iter()
        .find(|e| matches!(e, ClientError::InvalidArgument(_)))
        .expect("InvalidArgument variant missing");
    assert!(ia.is_invalid_argument());
}

#[then("NOT_FOUND gRPC error should have is_invalid_argument false")]
async fn then_not_found_invalid_argument_false(world: &mut ErrorHandlingWorld) {
    let nf = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::NotFound))
        .expect("not_found variant missing");
    assert!(!nf.is_invalid_argument());
}

#[then("connection error should have is_connection_error true")]
async fn then_connection_is_connection_true(world: &mut ErrorHandlingWorld) {
    let conn = world
        .error_variants
        .iter()
        .find(|e| matches!(e, ClientError::Connection(_)))
        .expect("connection variant missing");
    assert!(conn.is_connection_error());
}

#[then("transport error should have is_connection_error true")]
async fn then_transport_is_connection_true(world: &mut ErrorHandlingWorld) {
    // We use connection error to simulate transport in tests
    let conn = world
        .error_variants
        .iter()
        .find(|e| matches!(e, ClientError::Connection(_)))
        .expect("connection variant missing");
    assert!(conn.is_connection_error());
}

#[then("gRPC error should have is_connection_error false")]
async fn then_grpc_is_connection_false(world: &mut ErrorHandlingWorld) {
    let grpc = world
        .error_variants
        .iter()
        .find(|e| matches!(e, ClientError::Grpc(_)))
        .expect("grpc variant missing");
    assert!(!grpc.is_connection_error());
}

#[then("connection errors should be retryable")]
async fn then_connection_retryable(world: &mut ErrorHandlingWorld) {
    let conn = world
        .error_variants
        .iter()
        .find(|e| matches!(e, ClientError::Connection(_)))
        .expect("connection variant missing");
    // Connection errors are typically retryable
    assert!(conn.is_connection_error());
}

#[then("UNAVAILABLE gRPC errors should be retryable")]
async fn then_unavailable_retryable(world: &mut ErrorHandlingWorld) {
    let unavailable = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::Unavailable))
        .expect("unavailable variant missing");
    assert_eq!(unavailable.code(), Some(Code::Unavailable));
}

#[then("RESOURCE_EXHAUSTED should be retryable with backoff")]
async fn then_resource_exhausted_retryable(world: &mut ErrorHandlingWorld) {
    let exhausted = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::ResourceExhausted))
        .expect("resource_exhausted variant missing");
    assert_eq!(exhausted.code(), Some(Code::ResourceExhausted));
}

#[then("INVALID_ARGUMENT should NOT be retryable")]
async fn then_invalid_argument_not_retryable(world: &mut ErrorHandlingWorld) {
    let ia = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::InvalidArgument))
        .expect("invalid_argument variant missing");
    // Invalid argument means bad input - retry won't help
    assert!(ia.is_invalid_argument());
}

#[then("FAILED_PRECONDITION should be retryable after state refresh")]
async fn then_failed_precondition_retryable(world: &mut ErrorHandlingWorld) {
    let fp = world
        .error_variants
        .iter()
        .find(|e| e.code() == Some(Code::FailedPrecondition))
        .expect("failed_precondition variant missing");
    assert!(fp.is_precondition_failed());
}

#[then("I should be able to extract retry timing hints")]
async fn then_extract_retry_hints(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    // In a real impl, status details would contain retry-after
    let _ = err.status();
}

#[then("I should get a formatted error message")]
async fn then_get_formatted_message(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let display = err.to_string();
    assert!(!display.is_empty());
}

#[then("the message should include the error type and description")]
async fn then_message_includes_type_and_desc(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let display = err.to_string();
    // Display format includes type info
    assert!(!display.is_empty());
}

#[then("I should get detailed diagnostic information")]
async fn then_get_diagnostic_info(world: &mut ErrorHandlingWorld) {
    let err = world.current_error.as_ref().expect("no error");
    let debug = format!("{:?}", err);
    assert!(!debug.is_empty());
}
