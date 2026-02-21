# docs:start:aggregate_client_contract
Feature: AggregateClient - Command Execution
  The AggregateClient sends commands to aggregates for processing.
  Commands are validated, processed, and result in events being persisted.
  Supports async (fire-and-forget), sync, and speculative modes.

  Without command execution, the system cannot accept user actions or
  change aggregate state.
# docs:end:aggregate_client_contract

  Background:
    Given an AggregateClient connected to the test backend

  # ==========================================================================
  # Basic Command Execution
  # ==========================================================================

  # docs:start:client_command
  Scenario: Execute command on new aggregate
    Given a new aggregate root in domain "orders"
    When I execute a "CreateOrder" command with data "customer-123"
    Then the command should succeed
    And the response should contain 1 event
    And the event should have type "OrderCreated"

  Scenario: Execute command on existing aggregate
    Given an aggregate "orders" with root "order-001" at sequence 3
    When I execute a "AddItem" command at sequence 3
    Then the command should succeed
    And the response should contain events starting at sequence 3

  Scenario: Execute command with correlation ID
    Given a new aggregate root in domain "orders"
    When I execute a command with correlation ID "trace-456"
    Then the command should succeed
    And the response events should have correlation ID "trace-456"
  # docs:end:client_command

  # ==========================================================================
  # Optimistic Concurrency
  # ==========================================================================

  # docs:start:client_concurrency
  Scenario: Command at wrong sequence fails with precondition error
    Given an aggregate "orders" with root "order-002" at sequence 5
    When I execute a command at sequence 3
    Then the command should fail with precondition error
    And the error should indicate sequence mismatch

  Scenario: Concurrent writes are detected
    Given an aggregate "orders" with root "order-003" at sequence 0
    When two commands are sent concurrently at sequence 0
    Then one should succeed
    And one should fail with precondition error

  Scenario: Retry with correct sequence succeeds
    Given an aggregate "orders" with root "order-004" at sequence 5
    When I execute a command at sequence 3
    Then the command should fail with precondition error
    When I query the current sequence for "orders" root "order-004"
    And I retry the command at the correct sequence
    Then the command should succeed
  # docs:end:client_concurrency

  # ==========================================================================
  # Sync Modes
  # ==========================================================================

  Scenario: Async command returns immediately
    Given a new aggregate root in domain "orders"
    When I execute a command asynchronously
    Then the response should return without waiting for projectors

  Scenario: Sync SIMPLE waits for projectors
    Given a new aggregate root in domain "orders"
    And projectors are configured for "orders" domain
    When I execute a command with sync mode SIMPLE
    Then the response should include projector results

  Scenario: Sync CASCADE waits for saga chain
    Given a new aggregate root in domain "orders"
    And sagas are configured for "orders" domain
    When I execute a command with sync mode CASCADE
    Then the response should include downstream saga results

  # ==========================================================================
  # Command Validation
  # ==========================================================================

  Scenario: Invalid command payload returns error
    Given an aggregate "orders" with root "order-005"
    When I execute a command with malformed payload
    Then the command should fail with invalid argument error

  Scenario: Missing required fields returns error
    Given a new aggregate root in domain "orders"
    When I execute a command without required fields
    Then the command should fail with invalid argument error
    And the error message should describe the missing field

  Scenario: Command to non-existent domain returns error
    When I execute a command to domain "nonexistent"
    Then the command should fail
    And the error should indicate unknown domain

  # ==========================================================================
  # Multi-Event Commands
  # ==========================================================================

  Scenario: Command can produce multiple events
    Given an aggregate "orders" with root "order-006" at sequence 0
    When I execute a command that produces 3 events
    Then the response should contain 3 events
    And events should have sequences 0, 1, 2

  Scenario: Multi-event command is atomic
    Given an aggregate "orders" with root "order-007" at sequence 0
    When I execute a command that produces 3 events
    And I query events for "orders" root "order-007"
    Then I should see all 3 events or none

  # ==========================================================================
  # Connection Handling
  # ==========================================================================

  Scenario: Connection failure returns error
    Given the aggregate service is unavailable
    When I attempt to execute a command
    Then the operation should fail with connection error

  Scenario: Timeout returns error
    Given the aggregate service is slow to respond
    When I execute a command with timeout 100ms
    Then the operation should fail with timeout or deadline error

  # ==========================================================================
  # New Aggregate Creation
  # ==========================================================================

  Scenario: First command creates aggregate implicitly
    Given no aggregate exists for domain "orders" root "order-new"
    When I execute a "CreateOrder" command for root "order-new" at sequence 0
    Then the command should succeed
    And the aggregate should now exist with 1 event

  Scenario: First command must be at sequence 0
    Given no aggregate exists for domain "orders" root "order-new2"
    When I execute a command at sequence 5
    Then the command should fail with precondition error
