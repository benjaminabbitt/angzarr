Feature: Speculative Component Execution
  Speculative execution reuses the same registered client logic handlers
  as normal execution. No duplication, no forked paths. The framework
  controls side effects: projectors skip writes, sagas return commands
  without executing them, and process managers return events without
  persisting them.

  Background:
    # Tests run against standalone mode
    # Speculative execution is only available in-process

  # ===========================================================================
  # Projector Speculative Execution
  # ===========================================================================

  @e2e @speculative @projector
  Scenario: Speculative projector returns projection without side effects
    Given an order "ORD-SPEC-PROJ" exists with subtotal 5000 cents
    When I speculatively run the "web" projector against order "ORD-SPEC-PROJ" events
    Then the speculative projection succeeds
    And speculative execution did not modify the web projector for "ORD-SPEC-PROJ"

  @e2e @speculative @projector
  Scenario: Speculative projector produces identical result to normal execution
    Given an order "ORD-SPEC-COMPARE" exists with subtotal 3000 cents
    When I speculatively run the "accounting" projector against order "ORD-SPEC-COMPARE" events
    Then the speculative projection succeeds

  # ===========================================================================
  # Saga Speculative Execution
  # ===========================================================================

  @e2e @speculative @saga
  Scenario: Speculative saga returns commands without executing them
    Given an order "ORD-SPEC-SAGA" exists and is paid
    When I speculatively run the "fulfillment-saga" against order "ORD-SPEC-SAGA" completion events
    Then the speculative saga produces commands
    And no fulfillment events exist for "ORD-SPEC-SAGA"

  @e2e @speculative @saga
  Scenario: Speculative saga with domain state specs
    Given a customer "CUST-SPEC-LOYAL" with 500 loyalty points
    And an order "ORD-SPEC-LOYAL" for customer "CUST-SPEC-LOYAL" totaling 5000 cents
    When I speculatively run the "loyalty-earn-saga" against order "ORD-SPEC-LOYAL" completion events with current state
    Then the speculative saga produces commands
    And customer "CUST-SPEC-LOYAL" loyalty points are unchanged

  # ===========================================================================
  # Process Manager Speculative Execution
  # ===========================================================================

  @e2e @speculative @pm
  Scenario: Speculative PM returns commands and events without persistence
    Given an order "ORD-SPEC-PM" exists and is paid
    When I speculatively run the "order-fulfillment" PM against order "ORD-SPEC-PM" completion events
    Then the speculative PM produces a result
    And no process manager events are persisted for "ORD-SPEC-PM"

  # ===========================================================================
  # No Side Effects Verification
  # ===========================================================================

  @e2e @speculative @no-side-effects
  Scenario: Speculative execution does not persist events
    Given an order "ORD-SPEC-CLEAN" exists with subtotal 2000 cents
    And I record the event count for order "ORD-SPEC-CLEAN"
    When I speculatively run the "web" projector against order "ORD-SPEC-CLEAN" events
    And I speculatively run the "fulfillment-saga" against order "ORD-SPEC-CLEAN" completion events
    Then the event count for order "ORD-SPEC-CLEAN" is unchanged
