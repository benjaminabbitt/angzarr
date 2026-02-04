Feature: Complete Order Lifecycle
  Tests the full business flow from order creation through fulfillment,
  validating correlation ID propagation across all domains.

  Background:
    # Tests run against standalone mode with SQLite storage

  # ===========================================================================
  # Order Operations
  # ===========================================================================

  @e2e @flow @order
  Scenario: Create an order
    When I create order "ORD-001" with item "WIDGET-001" quantity 2 and correlation "CORR-001"
    Then the command succeeds
    And an event "OrderCreated" is emitted
    And the correlation ID "CORR-001" is in the response

  @e2e @flow @order
  Scenario: Submit payment for order
    Given an order "ORD-PAY" exists
    When I submit payment of 5000 cents via "card" for order "ORD-PAY"
    Then the command succeeds
    And an event "PaymentSubmitted" is emitted

  @e2e @flow @order
  Scenario: Complete order payment
    Given an order "ORD-COMPLETE" exists and is paid
    When I confirm payment for order "ORD-COMPLETE" with reference "PAY-REF-001"
    Then the command succeeds
    And an event "OrderCompleted" is emitted

  @e2e @flow @order
  Scenario: Cancel an order
    Given an order "ORD-CANCEL" exists
    When I cancel order "ORD-CANCEL" with reason "Customer request"
    Then the command succeeds
    And an event "OrderCancelled" is emitted

  # ===========================================================================
  # Full Business Flow
  # ===========================================================================

  @e2e @flow @full
  Scenario: Complete order lifecycle with correlation tracking
    # Setup inventory
    Given inventory for "SKU-FLOW-001" has 100 units

    # Order phase
    When I create order "FULL-ORDER" with item "SKU-FLOW-001" quantity 3 and correlation "FULL-CORR"
    Then the command succeeds

    # Inventory reservation via saga
    Then within 5 seconds a "StockReserved" event is emitted for product "SKU-FLOW-001"

    # Payment
    When I submit payment of 7500 cents via "card" for order "FULL-ORDER" with correlation "FULL-CORR"
    Then the command succeeds

    When I confirm payment for order "FULL-ORDER" with reference "PAY-FULL" and correlation "FULL-CORR"
    Then the command succeeds

    # Verify correlation ID appears in all events
    Then correlation "FULL-CORR" appears in events:
      | event_type        |
      | OrderCreated      |
      | PaymentSubmitted  |
      | OrderCompleted    |

  @e2e @flow @full
  Scenario: Multiple orders with independent correlation IDs
    Given inventory for "SKU-MULTI" has 200 units

    # First order
    When I create order "MULTI-ORD-1" with item "SKU-MULTI" quantity 1 and correlation "MULTI-CORR-1"
    Then the command succeeds

    # Second order (different correlation)
    When I create order "MULTI-ORD-2" with item "SKU-MULTI" quantity 2 and correlation "MULTI-CORR-2"
    Then the command succeeds

    # Verify isolation
    Then correlation "MULTI-CORR-1" only appears in order "MULTI-ORD-1" events
    And correlation "MULTI-CORR-2" only appears in order "MULTI-ORD-2" events

  # ===========================================================================
  # Error Handling
  # ===========================================================================

  @e2e @flow @errors
  Scenario: Cannot complete unpaid order
    Given an order "ORD-UNPAID" exists without payment
    When I confirm payment for order "ORD-UNPAID" with reference "PAY-ERR"
    Then the command fails with "not paid"

  @e2e @flow @errors
  Scenario: Cannot cancel completed order
    Given an order "ORD-DONE" exists and is completed
    When I cancel order "ORD-DONE" with reason "Too late"
    Then the command fails with "completed"

  # ===========================================================================
  # Full End-to-End: Order to Delivery (all sagas + PMs, no manual bridging)
  # ===========================================================================

  @e2e @flow @e2e-full
  Scenario: Complete order lifecycle through all sagas and process managers
    # Setup: inventory at deterministic root
    Given inventory for "E2E-WIDGET" has 100 units

    # Phase 1: Create order + inventory reservation saga
    When I create order "E2E-ORDER" with item "E2E-WIDGET" quantity 2 and correlation "E2E-CORR"
    Then the command succeeds
    And within 5 seconds a "StockReserved" event is emitted for product "E2E-WIDGET"

    # Phase 2: Payment
    When I submit payment of 5000 cents via "card" for order "E2E-ORDER" with correlation "E2E-CORR"
    Then the command succeeds

    When I confirm payment for order "E2E-ORDER" with reference "E2E-PAY-001" and correlation "E2E-CORR"
    Then the command succeeds

    # Phase 3: Post-completion sagas fire
    Then within 10 seconds the correlation "E2E-CORR" appears in:
      | domain      | event_type         |
      | order       | OrderCompleted     |
      | fulfillment | ShipmentCreated    |

    # Phase 4: Fulfillment + PM dispatch
    And the saga-created shipment for correlation "E2E-CORR" is stored as "E2E-SHIP"
    When I mark shipment "E2E-SHIP" as picked by "PICKER-E2E" with correlation "E2E-CORR"
    Then the command succeeds
    When I mark shipment "E2E-SHIP" as packed by "PACKER-E2E" with correlation "E2E-CORR"
    Then the command succeeds

    # PM sees all 3: PaymentSubmitted + StockReserved + ItemsPacked -> Ship
    Then within 10 seconds the correlation "E2E-CORR" appears in:
      | domain      | event_type |
      | fulfillment | Shipped    |

    # Phase 5: Delivery
    When I record delivery for "E2E-SHIP" with signature "Alice" and correlation "E2E-CORR"
    Then the command succeeds
    And within 5 seconds the correlation "E2E-CORR" appears in:
      | domain      | event_type |
      | fulfillment | Delivered  |
