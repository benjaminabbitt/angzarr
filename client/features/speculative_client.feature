Feature: SpeculativeClient - What-If Execution
  The SpeculativeClient enables "what-if" scenarios without persistence.
  Commands, projectors, sagas, and process managers can be executed
  speculatively to preview outcomes before committing.

  Use cases:
  - UI previews: Show user what will happen before confirming
  - Validation: Check if a command would succeed before sending
  - Testing: Verify behavior without side effects
  - Simulation: Model hypothetical scenarios

  Background:
    Given a SpeculativeClient connected to the test backend

  # ==========================================================================
  # Speculative Aggregate Execution
  # ==========================================================================

  Scenario: Speculative command returns projected events
    Given an aggregate "orders" with root "order-001" has 3 events
    When I speculatively execute a command against "orders" root "order-001"
    Then the response should contain the projected events
    And the events should NOT be persisted

  Scenario: Speculative command respects temporal query
    Given an aggregate "orders" with root "order-002" has 10 events
    When I speculatively execute a command as of sequence 5
    Then the command should execute against the historical state
    And the response should reflect state at sequence 5

  Scenario: Speculative command validates business rules
    Given an aggregate "orders" with root "order-003" in state "shipped"
    When I speculatively execute a "CancelOrder" command
    Then the response should indicate rejection
    And the rejection reason should be "cannot cancel shipped order"

  Scenario: Speculative command with invalid input fails fast
    Given an aggregate "orders" with root "order-004"
    When I speculatively execute a command with invalid payload
    Then the operation should fail with validation error
    And no events should be produced

  Scenario: Speculative execution creates implicit edition
    Given an aggregate "orders" with root "order-005" has 5 events
    When I speculatively execute a command
    Then an edition should be created for the speculation
    And the edition should be discarded after execution

  # ==========================================================================
  # Speculative Projector Execution
  # ==========================================================================

  Scenario: Speculative projector returns projection without side effects
    Given events for "orders" root "order-006"
    When I speculatively execute projector "order-summary" against those events
    Then the response should contain the projection
    And no external systems should be updated

  Scenario: Speculative projector handles event sequence
    Given 5 events for "orders" root "order-007"
    When I speculatively execute projector "order-summary"
    Then the projector should process all 5 events in order
    And the final projection state should be returned

  # ==========================================================================
  # Speculative Saga Execution
  # ==========================================================================

  Scenario: Speculative saga returns commands without sending
    Given events for "orders" root "order-008"
    When I speculatively execute saga "order-fulfillment"
    Then the response should contain the commands the saga would emit
    And the commands should NOT be sent to the target domain

  Scenario: Speculative saga respects saga origin
    Given events with saga origin from "inventory" aggregate
    When I speculatively execute saga "inventory-order"
    Then the response should preserve the saga origin chain

  # ==========================================================================
  # Speculative Process Manager Execution
  # ==========================================================================

  Scenario: Speculative PM returns orchestrated commands
    Given correlated events from multiple domains
    When I speculatively execute process manager "order-workflow"
    Then the response should contain the PM's command decisions
    And the commands should NOT be executed

  Scenario: Speculative PM requires correlation ID
    Given events without correlation ID
    When I speculatively execute process manager "order-workflow"
    Then the operation should fail
    And the error should indicate missing correlation ID

  # ==========================================================================
  # State Isolation
  # ==========================================================================

  Scenario: Speculative execution does not affect real state
    Given an aggregate "orders" with root "order-009" has 3 events
    When I speculatively execute a command producing 2 events
    And I query events for "orders" root "order-009"
    Then I should receive only 3 events
    And the speculative events should not be present

  Scenario: Multiple speculative executions are independent
    Given an aggregate "orders" with root "order-010" has 3 events
    When I speculatively execute command A
    And I speculatively execute command B
    Then each speculation should start from the same base state
    And results should be independent

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Speculative execution of unavailable service fails
    Given the speculative service is unavailable
    When I attempt speculative execution
    Then the operation should fail with connection error

  Scenario: Invalid speculative request returns error
    When I attempt speculative execution with missing parameters
    Then the operation should fail with invalid argument error
