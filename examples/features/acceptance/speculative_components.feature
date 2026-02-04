Feature: Speculative Component Execution
  Speculative execution reuses the same registered client logic handlers
  as normal execution. No duplication, no forked paths. The framework
  controls side effects: projectors skip writes, sagas return commands
  without executing them, and process managers return events without
  persisting them.

  Background:
    # Saga/PM speculation requires standalone mode (direct access)

  # ===========================================================================
  # Saga Speculative Execution
  # ===========================================================================

  @e2e @speculative @saga @standalone
  Scenario: Speculative saga returns commands without executing them
    Given an order "ORD-SPEC-SAGA" exists and is paid
    When I speculatively run the "order-fulfillment-saga" against order "ORD-SPEC-SAGA" completion events
    Then the speculative saga produces commands
    And no fulfillment events exist for "ORD-SPEC-SAGA"

  # ===========================================================================
  # Process Manager Speculative Execution
  # ===========================================================================

  @e2e @speculative @pm @standalone
  Scenario: Speculative PM returns commands and events without persistence
    Given an order "ORD-SPEC-PM" exists and is paid
    When I speculatively run the "order-fulfillment" PM against order "ORD-SPEC-PM" completion events
    Then the speculative PM produces a result
    And no process manager events are persisted for "ORD-SPEC-PM"

  # ===========================================================================
  # No Side Effects Verification
  # ===========================================================================

  @e2e @speculative @no-side-effects @standalone
  Scenario: Speculative execution does not persist events
    Given an order "ORD-SPEC-CLEAN" exists with subtotal 2000 cents
    And I record the event count for order "ORD-SPEC-CLEAN"
    When I speculatively run the "inventory" projector against order "ORD-SPEC-CLEAN" events
    And I speculatively run the "order-fulfillment-saga" against order "ORD-SPEC-CLEAN" completion events
    Then the event count for order "ORD-SPEC-CLEAN" is unchanged
