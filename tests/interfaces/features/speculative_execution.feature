Feature: Speculative execution
  Speculative execution runs handler logic without side effects, enabling
  "what-if" queries and preview functionality. The framework suppresses
  persistence, publishing, and command execution while handlers remain
  unaware of the execution mode.

  Use cases:
  - Preview saga output before committing
  - Test PM behavior with hypothetical events
  - Validate projector transformations without updating read models
  - Debug handler logic with controlled state

  Key behaviors:
  - Same handler instances used (no duplication)
  - No persistence (events not stored)
  - No publishing (events not broadcast)
  - No command execution (commands returned but not executed)
  - State resolution supports current, at-sequence, at-timestamp, explicit

  Background:
    Given a speculative execution test environment

  # ==========================================================================
  # Domain State Resolution
  # ==========================================================================

  Scenario: Current state resolves latest events
    Given an aggregate "orders" with root "order-123" has 5 events
    When I resolve state with DomainStateSpec::Current
    Then the resolved state contains 5 events

  Scenario: At-sequence resolves events up to that sequence
    Given an aggregate "orders" with root "order-123" has 10 events
    When I resolve state with DomainStateSpec::AtSequence(5)
    Then the resolved state contains 6 events
    And the last event has sequence 5

  Scenario: At-timestamp resolves events up to that time
    Given an aggregate "orders" with root "order-123" has 3 events with timestamps
    When I resolve state with AtTimestamp("2024-01-01T11:30:00Z")
    Then the resolved state contains 2 events
    And the last event has sequence 1

  Scenario: Explicit state uses provided EventBook directly
    Given I provide an explicit EventBook with 3 events
    When I resolve state with DomainStateSpec::Explicit
    Then the resolved state is the provided EventBook
    And no storage queries are made

  # ==========================================================================
  # Speculative Projector Execution
  # ==========================================================================

  Scenario: Speculative projector returns projection without persisting
    Given a projector "order-summary" is registered
    And an event book with an OrderPlaced event
    When I speculatively execute the projector
    Then I receive a Projection result
    And the projector's read model is not updated

  Scenario: Speculative projector by domain routes correctly
    Given a projector handles domain "orders"
    And an event book from domain "orders"
    When I speculatively execute projector by domain "orders"
    Then the projector handler is invoked
    And the projection mode is Speculate

  Scenario: Speculative projector not found returns error
    When I speculatively execute projector "nonexistent"
    Then I receive a NotFound error
    And the error message contains "No projector registered"

  # ==========================================================================
  # Speculative Saga Execution
  # ==========================================================================

  Scenario: Speculative saga returns commands without executing
    Given a saga "order-fulfillment" is registered
    And a source event book with an OrderCompleted event
    When I speculatively execute the saga
    Then I receive command books as output
    And no commands are executed
    And no events are persisted

  Scenario: Speculative saga resolves destinations from spec
    Given a saga that needs destination "inventory" state
    And DomainStateSpec for "inventory" is Current
    When I speculatively execute the saga
    Then the saga receives the inventory EventBook

  Scenario: Speculative saga by source domain routes correctly
    Given a saga handles source domain "orders"
    And a source event book from domain "orders"
    When I speculatively execute saga by source domain "orders"
    Then the saga handler is invoked

  Scenario: Speculative saga with missing destination falls back to current
    Given a saga that needs destination "inventory" state
    And no DomainStateSpec is provided for "inventory"
    When I speculatively execute the saga
    Then a warning is logged about missing domain_spec
    And the saga receives current inventory state

  # ==========================================================================
  # Speculative Process Manager Execution
  # ==========================================================================

  Scenario: Speculative PM returns commands and events without persistence
    Given a process manager "order-flow" is registered
    And a trigger event book
    When I speculatively execute the PM
    Then I receive PmSpeculativeResult with:
      | commands       | 1 or more |
      | process_events | optional  |
      | facts          | 0 or more |
    And no PM events are persisted
    And no commands are executed

  Scenario: Speculative PM resolves PM state by correlation
    Given a PM "order-flow" with existing state for correlation "txn-123"
    And a trigger event with correlation "txn-123"
    When I speculatively execute the PM
    Then the PM receives its previous state

  Scenario: Speculative PM by trigger domain routes correctly
    Given a PM subscribes to domain "orders"
    And a trigger event from domain "orders"
    When I speculatively execute PM by trigger domain "orders"
    Then the PM handler is invoked

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Missing storage for domain returns error
    Given no storage is configured for domain "unknown"
    When I resolve state for domain "unknown"
    Then I receive a NotFound error
    And the error message contains "No storage configured"

  Scenario: Invalid root UUID in cover returns error
    Given a cover with invalid root UUID bytes
    When I resolve destinations with that cover
    Then I receive an InvalidArgument error
    And the error message contains "Invalid root UUID"

  # ==========================================================================
  # Handler Instance Reuse
  # ==========================================================================

  Scenario: Speculative execution uses same handler instances
    Given a saga "order-fulfillment" registered with the runtime
    When I speculatively execute that saga
    Then the same handler instance is invoked
    And any handler-internal state is shared
