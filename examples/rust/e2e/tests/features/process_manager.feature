Feature: Process Manager - Order Fulfillment
  Tests the order fulfillment process manager which coordinates across
  order, inventory, and fulfillment domains using fan-in pattern.

  The PM tracks three prerequisites before dispatching Ship:
  - PaymentSubmitted (from order domain)
  - StockReserved (from inventory domain)
  - ItemsPacked (from fulfillment domain, after pick and pack)

  Background:
    # Process manager "order-fulfillment" subscribes to events from
    # order, inventory, and fulfillment domains.
    # PM state is stored in its own "order-fulfillment" domain.

  # ===========================================================================
  # Fan-In Coordination
  # ===========================================================================

  @e2e @pm @fanin
  Scenario: All three prerequisites trigger Ship command
    Given an order "ORD-PM-001" with correlation "PM-CORR-001"
    When PaymentSubmitted is received for correlation "PM-CORR-001"
    Then no Ship command is dispatched yet

    When StockReserved is received for correlation "PM-CORR-001"
    Then no Ship command is dispatched yet

    When ItemsPacked is received for correlation "PM-CORR-001"
    Then within 5 seconds a Ship command is dispatched to fulfillment
    And the Ship command has correlation "PM-CORR-001"

  @e2e @pm @fanin
  Scenario: Prerequisites arrive in reverse order
    Given an order "ORD-PM-002" with correlation "PM-CORR-002"
    When ItemsPacked arrives first for correlation "PM-CORR-002"
    And StockReserved arrives second for correlation "PM-CORR-002"
    And PaymentSubmitted arrives third for correlation "PM-CORR-002"
    Then within 5 seconds a Ship command is dispatched to fulfillment

  @e2e @pm @fanin
  Scenario: Prerequisites arrive with inventory first
    Given an order "ORD-PM-003" with correlation "PM-CORR-003"
    When StockReserved arrives for correlation "PM-CORR-003"
    And PaymentSubmitted arrives for correlation "PM-CORR-003"
    And ItemsPacked arrives for correlation "PM-CORR-003"
    Then within 5 seconds a Ship command is dispatched to fulfillment

  # ===========================================================================
  # Idempotency
  # ===========================================================================

  @e2e @pm @idempotency
  Scenario: Duplicate event does not trigger extra Ship
    Given all prerequisites completed for correlation "PM-CORR-IDEM"
    And Ship was already dispatched
    When a duplicate PaymentSubmitted arrives for correlation "PM-CORR-IDEM"
    Then no additional Ship command is dispatched

  @e2e @pm @idempotency
  Scenario: Re-delivered ItemsPacked after Ship is no-op
    Given all prerequisites completed for correlation "PM-CORR-IDEM-2"
    And Ship was already dispatched
    When ItemsPacked is re-delivered for correlation "PM-CORR-IDEM-2"
    Then no additional Ship command is dispatched

  # ===========================================================================
  # Process Manager State
  # ===========================================================================

  @e2e @pm @state
  Scenario: PM state tracks partial progress
    Given an order "ORD-PM-STATE" with correlation "PM-CORR-STATE"
    When PaymentSubmitted is received for correlation "PM-CORR-STATE"
    Then querying PM state for correlation "PM-CORR-STATE" shows:
      | prerequisite     | status    |
      | payment          | completed |
      | inventory        | pending   |
      | fulfillment      | pending   |

  @e2e @pm @state
  Scenario: PM state shows all complete before Ship
    Given all prerequisites received for correlation "PM-CORR-FULL"
    Then querying PM state for correlation "PM-CORR-FULL" shows:
      | prerequisite     | status    |
      | payment          | completed |
      | inventory        | completed |
      | fulfillment      | completed |
      | dispatched       | completed |

  # ===========================================================================
  # Correlation Isolation
  # ===========================================================================

  @e2e @pm @isolation
  Scenario: Different orders have independent PM state
    Given orders with correlations "PM-ISO-A" and "PM-ISO-B"
    When PaymentSubmitted arrives for "PM-ISO-A"
    And all three prerequisites arrive for "PM-ISO-B"
    Then Ship is dispatched only for "PM-ISO-B"
    And PM state for "PM-ISO-A" shows only payment completed

  # ===========================================================================
  # Integration with Full Flow
  # ===========================================================================

  @e2e @pm @integration
  Scenario: PM triggers Ship in complete order lifecycle
    # This tests the PM as part of the full chain:
    # cart checkout → order created → payment submitted → stock reserved
    # → fulfillment saga creates shipment → pick → pack → PM receives all three → Ship
    Given a product "PM-WIDGET" with price 2500 and stock 100
    And a customer "PM-ALICE" exists
    When customer "PM-ALICE" creates and checks out a cart with "PM-WIDGET"
    And payment is submitted for the order
    And stock is reserved for the order
    Then within 10 seconds the fulfillment process dispatches Ship
    And the shipment transitions to "shipped" status
