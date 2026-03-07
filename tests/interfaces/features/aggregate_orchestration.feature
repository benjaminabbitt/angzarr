Feature: Aggregate command orchestration
  The aggregate orchestrator coordinates command execution including:
  - Merge strategy enforcement (concurrency control)
  - Fact injection (external event ingestion)
  - Command rejection handling

  Without proper orchestration, concurrent commands could corrupt aggregate state,
  external events would have no ingestion path, and rejections would not propagate
  correctly for compensation flows.

  Background:
    Given an aggregate orchestration test environment

  # ==========================================================================
  # Merge Strategy: STRICT (Optimistic Concurrency)
  # ==========================================================================
  # Use when commands MUST see latest state before execution.
  # Rejects immediately on sequence mismatch.

  Scenario: STRICT accepts command at correct sequence
    Given an aggregate "orders" with 3 prior events
    And a command with merge_strategy STRICT targeting sequence 3
    When the orchestrator executes the command
    Then the command succeeds
    And the produced events are persisted
    And the aggregate has 4 events

  Scenario: STRICT rejects command at stale sequence
    Given an aggregate "orders" with 3 prior events
    And a command with merge_strategy STRICT targeting sequence 1
    When the orchestrator executes the command
    Then the command fails
    And the error indicates sequence mismatch
    And no new events are persisted

  Scenario: STRICT rejects command at future sequence
    Given an aggregate "orders" with 3 prior events
    And a command with merge_strategy STRICT targeting sequence 10
    When the orchestrator executes the command
    Then the command fails
    And the error indicates sequence mismatch

  # ==========================================================================
  # Merge Strategy: COMMUTATIVE (Retryable)
  # ==========================================================================
  # Use when commands can be safely re-executed with fresh state.
  # Returns retryable error with current state attached.

  Scenario: COMMUTATIVE accepts command at correct sequence
    Given an aggregate "inventory" with 2 prior events
    And a command with merge_strategy COMMUTATIVE targeting sequence 2
    When the orchestrator executes the command
    Then the command succeeds
    And the produced events are persisted

  Scenario: COMMUTATIVE returns retryable error on stale sequence
    Given an aggregate "inventory" with 5 prior events
    And a command with merge_strategy COMMUTATIVE targeting sequence 2
    When the orchestrator executes the command
    Then the command fails with retryable status
    And the error includes the current EventBook
    And the EventBook shows next_sequence 5

  Scenario: COMMUTATIVE is the default strategy
    Given an aggregate "inventory" with no prior events
    And a command with no explicit merge_strategy
    When the orchestrator executes the command
    Then the effective merge_strategy is COMMUTATIVE

  # ==========================================================================
  # Merge Strategy: AGGREGATE_HANDLES (Delegate to Business Logic)
  # ==========================================================================
  # Use when aggregate has domain-specific concurrency logic.
  # Coordinator skips sequence validation entirely.

  Scenario: AGGREGATE_HANDLES bypasses coordinator sequence validation
    Given an aggregate "counters" with 10 prior events
    And a command with merge_strategy AGGREGATE_HANDLES targeting sequence 0
    When the orchestrator executes the command
    Then the aggregate handler is invoked
    And the handler receives the prior EventBook
    And the command succeeds

  Scenario: AGGREGATE_HANDLES lets aggregate reject based on state
    Given an aggregate "counters" with 10 prior events
    And a command with merge_strategy AGGREGATE_HANDLES targeting sequence 0
    And the aggregate will reject due to state conflict
    When the orchestrator executes the command
    Then the command fails with aggregate's rejection
    And no new events are persisted

  # ==========================================================================
  # Fact Injection
  # ==========================================================================
  # Facts are external events injected directly, bypassing command validation.
  # Used for external realities the aggregate must accept (e.g., "hand says it's your turn").

  Scenario: Fact injection persists external events
    Given an aggregate "game" with no prior events
    And a fact with external_id "external-001" and type "TurnAssigned"
    When the orchestrator injects the fact
    Then the fact is persisted as an event
    And the aggregate has 1 event
    And the event has type "TurnAssigned"

  Scenario: Fact injection with external_id is idempotent
    Given an aggregate "game" with no prior events
    And a fact with external_id "external-002" and type "TurnAssigned"
    When the orchestrator injects the fact
    And the orchestrator injects the same fact again
    Then the aggregate has 1 event
    And the second injection returns the original sequences

  Scenario: Fact injection assigns real sequence numbers
    Given an aggregate "game" with 5 prior events
    And a fact with external_id "external-003" and type "TurnAssigned"
    When the orchestrator injects the fact
    Then the fact is persisted at sequence 5
    And the aggregate has 6 events

  Scenario: Fact injection publishes to event bus
    Given an aggregate "game" with no prior events
    And a fact with external_id "external-004" and type "TurnAssigned"
    And a subscriber listening to domain "game"
    When the orchestrator injects the fact
    Then the subscriber receives the event

  # ==========================================================================
  # Command Rejection
  # ==========================================================================
  # Business logic can reject commands with meaningful errors.

  Scenario: Business logic rejection returns error
    Given an aggregate "payments" with no prior events
    And a command that business logic will reject
    When the orchestrator executes the command
    Then the command fails with business rejection
    And the error message contains the rejection reason
    And no events are persisted

  Scenario: Rejection does not affect aggregate state
    Given an aggregate "payments" with 3 prior events
    And a command that business logic will reject
    When the orchestrator executes the command
    Then the command fails
    And the aggregate still has 3 events

  # ==========================================================================
  # Event Publishing
  # ==========================================================================
  # After persistence, events are published to the bus for sagas/projectors.

  Scenario: Successful command publishes events to bus
    Given an aggregate "orders" with no prior events
    And a subscriber listening to domain "orders"
    And a command that produces an OrderCreated event
    When the orchestrator executes the command
    Then the subscriber receives the OrderCreated event

  Scenario: Failed command does not publish events
    Given an aggregate "orders" with 3 prior events
    And a subscriber listening to domain "orders"
    And a command with merge_strategy STRICT targeting sequence 0
    When the orchestrator executes the command
    Then the command fails
    And the subscriber receives no events

  # ==========================================================================
  # Sync Projector Integration
  # ==========================================================================

  Scenario: Sync projector receives events from command
    Given an aggregate "orders" with no prior events
    And a sync projector for domain "orders"
    And a command that produces events
    When the orchestrator executes the command
    Then the sync projector is invoked with the events
    And the projector output is included in the response

  Scenario: Sync projector not called when no events
    Given an aggregate "orders" with no prior events
    And a sync projector for domain "orders"
    And a command that produces no events
    When the orchestrator executes the command
    Then the sync projector is not invoked
