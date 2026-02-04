Feature: Speculative Component Execution
  Speculative execution reuses the same registered client logic handlers
  as normal execution. No duplication, no forked paths. The framework
  controls side effects: projectors skip writes, sagas return commands
  without executing them, and process managers return events without
  persisting them.

  Background:
    # Projector speculation works via gateway routing to coordinators
    # Saga/PM speculation requires standalone mode (direct access)

  # ===========================================================================
  # Projector Speculative Execution
  # ===========================================================================

  @e2e @speculative @projector
  Scenario: Speculative projector returns projection without side effects
    Given inventory for "INV-SPEC-PROJ" has 100 units
    When I speculatively run the "inventory" projector against inventory "INV-SPEC-PROJ" events
    Then the speculative projection succeeds
    And speculative execution did not modify the inventory projector for "INV-SPEC-PROJ"

  @e2e @speculative @projector
  Scenario: Speculative projector produces identical result to normal execution
    Given inventory for "INV-SPEC-COMPARE" has 50 units
    When I speculatively run the "inventory" projector against inventory "INV-SPEC-COMPARE" events
    Then the speculative projection succeeds

  # ===========================================================================
  # Saga Speculative Execution
  # ===========================================================================

  @e2e @speculative @saga @standalone
  Scenario: Speculative saga returns commands without executing them
    Given an order "ORD-SPEC-SAGA" exists and is paid
    When I speculatively run the "order-fulfillment-saga" against order "ORD-SPEC-SAGA" completion events
    Then the speculative saga produces commands
    And no fulfillment events exist for "ORD-SPEC-SAGA"

  @e2e @speculative @saga @standalone
  Scenario: Speculative saga with inventory reservation
    Given inventory for "SKU-SPEC-INV" has 100 units
    And an order "ORD-SPEC-INV" with item "SKU-SPEC-INV" quantity 5
    When I speculatively run the "inventory-reservation-saga" against order "ORD-SPEC-INV" creation events
    Then the speculative saga produces commands
    And inventory "SKU-SPEC-INV" available quantity is unchanged

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
