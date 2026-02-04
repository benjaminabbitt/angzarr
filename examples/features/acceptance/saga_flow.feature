Feature: Saga Orchestration
  Tests cross-domain saga event subscription and command generation.
  Validates that sagas correctly respond to domain events and generate
  commands with preserved correlation IDs.

  Background:
    # Tests run against standalone mode
    # Sagas are registered and listening to their trigger events

  # ===========================================================================
  # Fulfillment Saga
  # ===========================================================================

  @e2e @saga @fulfillment
  Scenario: Fulfillment saga triggers on order completion
    Given an order "ORD-FULFILL-001" exists and is paid
    When payment is confirmed for order "ORD-FULFILL-001" with correlation "CORR-FULFILL"
    Then within 5 seconds:
      | domain      | event_type      | correlation    |
      | order       | OrderCompleted  | CORR-FULFILL   |
      | fulfillment | ShipmentCreated | CORR-FULFILL   |
    And the shipment references order "ORD-FULFILL-001"

  @e2e @saga @fulfillment
  Scenario: Fulfillment saga creates correct shipment details
    Given an order "ORD-DETAILS" with items:
      | sku        | quantity |
      | WIDGET-001 | 2        |
      | WIDGET-002 | 3        |
    When the order "ORD-DETAILS" is completed with correlation "CORR-DETAILS"
    Then within 5 seconds a shipment is created
    And the shipment contains all order items
    And the correlation ID "CORR-DETAILS" is preserved in shipment events

  @e2e @saga @fulfillment
  Scenario: Multiple orders trigger independent fulfillments
    Given orders exist:
      | order_id      | status    |
      | ORD-MULTI-001 | paid      |
      | ORD-MULTI-002 | paid      |
    When I complete order "ORD-MULTI-001" with correlation "CORR-M1"
    And I complete order "ORD-MULTI-002" with correlation "CORR-M2"
    Then within 5 seconds 2 shipments are created
    And shipment for "ORD-MULTI-001" has correlation "CORR-M1"
    And shipment for "ORD-MULTI-002" has correlation "CORR-M2"

  # ===========================================================================
  # Inventory Reservation Saga
  # ===========================================================================

  @e2e @saga @inventory-reservation
  Scenario: Order creation reserves inventory
    Given inventory for "SKU-IR-001" has 100 units
    When I create order "ORD-IR-001" with item "SKU-IR-001" quantity 5 and correlation "CORR-IR-001"
    Then within 5 seconds a "StockReserved" event is emitted for product "SKU-IR-001"

  @e2e @saga @inventory-reservation
  Scenario: Order cancellation releases reservation
    Given inventory for "SKU-IR-002" has 100 units
    And an order "ORD-IR-002" with item "SKU-IR-002" quantity 3
    Then within 5 seconds a "StockReserved" event is emitted for product "SKU-IR-002"
    When I cancel order "ORD-IR-002" with correlation "CORR-IR-CANCEL"
    Then within 5 seconds a "ReservationReleased" event is emitted for product "SKU-IR-002"

  # ===========================================================================
  # Correlation ID Preservation
  # ===========================================================================

  @e2e @saga @correlation
  Scenario: Correlation ID preserved through fulfillment saga chain
    Given an order "ORD-CHAIN" with items totaling 5000 cents
    And the order is paid
    When order "ORD-CHAIN" is completed with correlation "CHAIN-CORR"
    Then within 5 seconds the correlation "CHAIN-CORR" appears in:
      | domain      | event_type         |
      | order       | OrderCompleted     |
      | fulfillment | ShipmentCreated    |

  @e2e @saga @correlation
  Scenario: Different orders maintain separate correlation IDs
    When I create order "ORD-A" with correlation "CORR-A"
    And I create order "ORD-B" with correlation "CORR-B"
    And both orders are completed
    Then fulfillment events for "ORD-A" have correlation "CORR-A"
    And fulfillment events for "ORD-B" have correlation "CORR-B"
    And no cross-contamination of correlation IDs

  # ===========================================================================
  # Saga Error Handling
  # ===========================================================================

  @e2e @saga @errors
  Scenario: Saga idempotency on duplicate events
    Given an order "ORD-IDEM" with items totaling 5000 cents
    And order "ORD-IDEM" is completed with correlation "IDEM-CORR"
    And within 5 seconds a shipment is created
    When I re-complete order "ORD-IDEM"
    Then the command fails with "completed"
    And no duplicate shipments exist

  # ===========================================================================
  # Saga Timing and Ordering
  # ===========================================================================

  @e2e @saga @timing
  Scenario: Async saga processing preserves correlation
    Given an order "ORD-ASYNC" with items totaling 3000 cents
    And the order is paid
    When order "ORD-ASYNC" is completed with correlation "ASYNC-CORR"
    Then within 5 seconds the correlation "ASYNC-CORR" appears in:
      | domain      | event_type         |
      | order       | OrderCompleted     |
      | fulfillment | ShipmentCreated    |
