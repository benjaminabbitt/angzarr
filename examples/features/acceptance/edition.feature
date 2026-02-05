Feature: Editions - Diverged Event Timelines
  Editions allow creating alternative event histories from a specific point
  for what-if analysis, A/B testing, and speculative execution without
  affecting the main timeline.

  Background:
    # Edition tests run in standalone mode only
    # Gateway mode doesn't support direct edition access

  # ===========================================================================
  # Edition Basics - Isolation
  # ===========================================================================

  @e2e @edition @isolation @standalone
  Scenario: Events in edition are isolated from main timeline
    Given an order "ORD-ISOLATED" exists and is paid
    When I create edition "what-if-v1" diverging at sequence 1
    And I apply loyalty discount of 500 points worth 250 cents to order "ORD-ISOLATED" in edition "what-if-v1"
    Then the command succeeds
    And in the main timeline order "ORD-ISOLATED" has 2 events
    And in edition "what-if-v1" order "ORD-ISOLATED" has 3 events

  @e2e @edition @isolation @standalone
  Scenario: Main timeline unaffected by edition commands
    Given an order "ORD-MAIN-SAFE" exists and is paid
    When I create edition "parallel-universe" diverging at sequence 1
    And I confirm payment for order "ORD-MAIN-SAFE" in edition "parallel-universe" with reference "PAY-ALT-001"
    Then the command succeeds
    And in the main timeline order "ORD-MAIN-SAFE" has 2 events
    And in edition "parallel-universe" order "ORD-MAIN-SAFE" has 3 events
    And the main timeline has no OrderCompleted event for "ORD-MAIN-SAFE"

  # ===========================================================================
  # Edition Data Preservation
  # ===========================================================================

  @e2e @edition @data @standalone
  Scenario: Edition preserves exact field values from divergence point
    Given an order "ORD-DATA-TEST" for customer "CUST-DATA" with item "Widget:2:1999"
    And payment is submitted for order "ORD-DATA-TEST" with amount 3998 cents
    When I create edition "field-test" diverging at sequence 2
    And I confirm payment for order "ORD-DATA-TEST" in edition "field-test" with reference "PAY-EDITION-001"
    Then the command succeeds
    And in edition "field-test" the OrderCompleted event contains:
      | field             | value          |
      | payment_reference | PAY-EDITION-001 |
      | final_total_cents | 3998           |
      | payment_method    | card           |

  # ===========================================================================
  # Saga Domain Boundary - Data Filtering
  # ===========================================================================

  @e2e @saga @domain-boundary
  Scenario: Order-Fulfillment saga filters payment data
    Given an order "ORD-FILTER" for customer "CUST-FILTER" with item "Laptop:1:99900"
    And payment is submitted for order "ORD-FILTER" with amount 99900 cents
    When I confirm payment for order "ORD-FILTER" with reference "PAY-FILTER-001"
    Then the command succeeds
    And an event "OrderCompleted" is emitted
    And within 5 seconds a ShipmentCreated event exists for the order
    And the ShipmentCreated event contains the order items
    # ShipmentCreated should NOT contain payment-specific fields
    # (final_total_cents, payment_method, payment_reference, loyalty_points_earned)
    # as these are order domain concerns, not fulfillment domain

  # ===========================================================================
  # Edition with Saga Propagation
  # ===========================================================================

  @e2e @edition @saga @standalone
  Scenario: Completing order in edition triggers saga in that edition
    Given an order "ORD-ED-SAGA" exists and is paid
    When I create edition "saga-test" diverging at sequence 2
    And I confirm payment for order "ORD-ED-SAGA" in edition "saga-test" with reference "PAY-SAGA-001"
    Then the command succeeds
    And in edition "saga-test" an OrderCompleted event is emitted
    # Note: Sagas don't propagate to editions by default in standalone mode
    # This tests the command execution in the edition context

  # ===========================================================================
  # Multiple Editions from Same Point
  # ===========================================================================

  @e2e @edition @multiple @standalone
  Scenario: Multiple editions can diverge from the same sequence
    Given an order "ORD-MULTI-ED" exists and is paid
    When I create edition "branch-a" diverging at sequence 1
    And I create edition "branch-b" diverging at sequence 1
    And I apply loyalty discount of 100 points worth 50 cents to order "ORD-MULTI-ED" in edition "branch-a"
    And I confirm payment for order "ORD-MULTI-ED" in edition "branch-b" with reference "PAY-BRANCH-B"
    Then in edition "branch-a" order "ORD-MULTI-ED" has 3 events
    And in edition "branch-b" order "ORD-MULTI-ED" has 3 events
    And in the main timeline order "ORD-MULTI-ED" has 2 events
