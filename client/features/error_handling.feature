# docs:start:error_handling_contract
Feature: Error Handling - Client Error Introspection
  Client errors provide structured information for retry logic,
  user feedback, and debugging. Errors are categorized by type
  (connection, validation, business rule) with introspection methods.

  Proper error handling enables:
  - Automatic retry on transient failures
  - User-friendly error messages
  - Optimistic concurrency conflict resolution
  - Debugging and logging
# docs:end:error_handling_contract

  # ==========================================================================
  # Error Categories
  # ==========================================================================

  Scenario: Connection error is identifiable
    Given the server is unreachable
    When I attempt a client operation
    Then the error should be a connection error
    And is_connection_error should return true
    And the error message should describe the connection failure

  Scenario: Transport error is identifiable
    Given the connection drops mid-request
    When I attempt a client operation
    Then the error should be a transport error
    And is_connection_error should return true

  Scenario: gRPC error wraps server status
    Given the server returns a gRPC error
    When I attempt a client operation
    Then the error should be a gRPC error
    And the underlying Status should be accessible

  Scenario: Invalid argument error from client validation
    When I build a command without required fields
    Then the error should be an invalid argument error
    And is_invalid_argument should return true
    And the error message should describe what's missing

  Scenario: Invalid timestamp error
    When I build a query with invalid timestamp format
    Then the error should be an invalid timestamp error
    And the error message should indicate the format problem

  # ==========================================================================
  # gRPC Error Introspection
  # ==========================================================================

  Scenario: NOT_FOUND error is identifiable
    Given the aggregate does not exist
    When I query events for the aggregate
    Then is_not_found should return true
    And code should return NOT_FOUND

  Scenario: FAILED_PRECONDITION error is identifiable
    Given an aggregate at sequence 5
    When I execute a command at sequence 3
    Then is_precondition_failed should return true
    And code should return FAILED_PRECONDITION
    And the error indicates optimistic lock failure

  Scenario: INVALID_ARGUMENT error from server
    When I send a malformed request to the server
    Then is_invalid_argument should return true
    And code should return INVALID_ARGUMENT

  Scenario: PERMISSION_DENIED error
    Given the client lacks required permissions
    When I attempt a restricted operation
    Then code should return PERMISSION_DENIED
    And the error message should describe access denial

  Scenario: INTERNAL error from server
    Given the server has an internal error
    When I attempt a client operation
    Then code should return INTERNAL
    And the error should indicate server-side failure

  Scenario: DEADLINE_EXCEEDED error
    Given the operation times out
    When I attempt a client operation
    Then code should return DEADLINE_EXCEEDED

  # ==========================================================================
  # Error Methods
  # ==========================================================================

  Scenario: message returns human-readable description
    Given any client error
    When I call message() on the error
    Then I should get a non-empty string
    And the message should describe the error

  Scenario: code returns gRPC code for gRPC errors
    Given a gRPC error with status NOT_FOUND
    When I call code() on the error
    Then I should get Some(NOT_FOUND)

  Scenario: code returns None for non-gRPC errors
    Given a connection error
    When I call code() on the error
    Then I should get None

  Scenario: status returns full Status for gRPC errors
    Given a gRPC error with detailed status
    When I call status() on the error
    Then I should get the full gRPC Status
    And I can access the status code, message, and details

  Scenario: status returns None for non-gRPC errors
    Given an invalid argument error
    When I call status() on the error
    Then I should get None

  # ==========================================================================
  # Boolean Predicates
  # ==========================================================================

  Scenario: is_not_found for various error types
    Given different error types
    Then NOT_FOUND gRPC error should have is_not_found true
    And connection error should have is_not_found false
    And INTERNAL gRPC error should have is_not_found false

  Scenario: is_precondition_failed for various error types
    Given different error types
    Then FAILED_PRECONDITION gRPC error should have is_precondition_failed true
    And NOT_FOUND gRPC error should have is_precondition_failed false
    And connection error should have is_precondition_failed false

  Scenario: is_invalid_argument from both sources
    Given different error types
    Then INVALID_ARGUMENT gRPC error should have is_invalid_argument true
    And ClientError::InvalidArgument should have is_invalid_argument true
    And NOT_FOUND gRPC error should have is_invalid_argument false

  Scenario: is_connection_error for connection types
    Given different error types
    Then connection error should have is_connection_error true
    And transport error should have is_connection_error true
    And gRPC error should have is_connection_error false

  # ==========================================================================
  # Retry Logic Support
  # ==========================================================================

  Scenario: Identify retryable errors
    Given various error types
    Then connection errors should be retryable
    And UNAVAILABLE gRPC errors should be retryable
    And RESOURCE_EXHAUSTED should be retryable with backoff
    And INVALID_ARGUMENT should NOT be retryable
    And FAILED_PRECONDITION should be retryable after state refresh

  Scenario: Extract retry information from error
    Given an error with retry-after metadata
    When I inspect the error details
    Then I should be able to extract retry timing hints

  # ==========================================================================
  # Error Display
  # ==========================================================================

  Scenario: Error implements Display
    Given any client error
    When I convert the error to string
    Then I should get a formatted error message
    And the message should include the error type and description

  Scenario: Error implements Debug
    Given any client error
    When I debug-format the error
    Then I should get detailed diagnostic information
