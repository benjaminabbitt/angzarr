Feature: Complete Order Lifecycle
  Tests the full business flow from order creation through fulfillment,
  validating correlation ID propagation across all domains.

  Background:
    # Tests run against standalone mode with SQLite storage

  # ===========================================================================
  # Order Operations
  # ===========================================================================

  @e2e @flow @order
  Scenario: Complete order payment
    Given an order "ORD-COMPLETE" exists and is paid
    When I confirm payment for order "ORD-COMPLETE" with reference "PAY-REF-001"
    Then the command succeeds
    And an event "OrderCompleted" is emitted

  # ===========================================================================
  # Error Handling
  # ===========================================================================

  @e2e @flow @errors
  Scenario: Cannot cancel completed order
    Given an order "ORD-DONE" exists and is completed
    When I cancel order "ORD-DONE" with reason "Too late"
    Then the command fails with "completed"
