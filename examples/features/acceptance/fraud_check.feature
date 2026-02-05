Feature: External Service Integration - Fraud Check
  Demonstrates aggregate command handlers calling external REST services.
  The fraud check is one example; this pattern applies to ANY external service:
  - Pricing services (dynamic pricing, MSRP lookup)
  - Tax services (calculate taxes by jurisdiction)
  - Address validation services
  - Payment gateways (pre-authorization, card verification)
  - Customer services (loyalty status, preferences)
  - Analytics/ML services (recommendations, predictions)
  - Inventory services (real-time availability)
  - Notification services (send confirmations)

  Background:
    # Mock fraud server is configured in standalone backend with:
    # - CUST-FRAUD -> declined
    # - CUST-REVIEW -> review_required
    # - All other customers -> approved (default)

  # ===========================================================================
  # Fraud Check - Approved
  # ===========================================================================

  @e2e @fraud @external-service
  Scenario: Payment confirmation includes approved fraud check result
    Given an order "ORD-FRAUD-OK" for customer "CUST-001" with item "Widget:2:1999"
    And payment is submitted for order "ORD-FRAUD-OK" with amount 3998 cents
    When I confirm payment for order "ORD-FRAUD-OK" with reference "PAY-FRAUD-001"
    Then the command succeeds
    And an event "OrderCompleted" is emitted
    And the OrderCompleted event has fraud_check_result "approved"

  # ===========================================================================
  # Fraud Check - Declined
  # ===========================================================================

  @e2e @fraud @external-service
  Scenario: Payment declined by fraud check
    Given an order "ORD-FRAUD-BAD" for customer "CUST-FRAUD" with item "Laptop:1:99900"
    And payment is submitted for order "ORD-FRAUD-BAD" with amount 99900 cents
    When I confirm payment for order "ORD-FRAUD-BAD" with reference "PAY-FRAUD-002"
    Then the command fails with "fraud"

  # ===========================================================================
  # Fraud Check - Review Required (still proceeds)
  # ===========================================================================

  @e2e @fraud @external-service
  Scenario: Payment with review_required fraud check still completes
    Given an order "ORD-FRAUD-REVIEW" for customer "CUST-REVIEW" with item "Gadget:1:5000"
    And payment is submitted for order "ORD-FRAUD-REVIEW" with amount 5000 cents
    When I confirm payment for order "ORD-FRAUD-REVIEW" with reference "PAY-FRAUD-003"
    Then the command succeeds
    And an event "OrderCompleted" is emitted
    And the OrderCompleted event has fraud_check_result "review_required"
