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
  # Loyalty Earn Saga
  # ===========================================================================

  @e2e @saga @loyalty
  Scenario: Loyalty points earned on order completion
    Given a customer "CUST-LOYAL" with 0 loyalty points
    And an order "ORD-LOYAL" for customer "CUST-LOYAL" totaling 5000 cents
    When order "ORD-LOYAL" is completed with correlation "CORR-LOYAL"
    Then within 5 seconds:
      | domain   | event_type    | correlation |
      | order    | OrderCompleted| CORR-LOYAL  |
      | customer | PointsEarned  | CORR-LOYAL  |
    And customer "CUST-LOYAL" has earned points based on order total

  # ===========================================================================
  # Cancellation Saga
  # ===========================================================================

  @e2e @saga @cancellation
  Scenario: Cancellation saga releases inventory and refunds points
    Given an order "ORD-CANCEL" with:
      | field              | value           |
      | customer_id        | CUST-CANCEL     |
      | loyalty_applied    | 100             |
      | inventory_reserved | true            |
    When order "ORD-CANCEL" is cancelled with reason "customer request" and correlation "CORR-CANCEL"
    Then within 5 seconds:
      | domain    | event_type          | correlation  |
      | order     | OrderCancelled      | CORR-CANCEL  |
      | inventory | ReservationReleased | CORR-CANCEL  |
      | customer  | PointsRefunded      | CORR-CANCEL  |
    And customer "CUST-CANCEL" has 100 points refunded

  @e2e @saga @cancellation
  Scenario: Cancellation without loyalty points skips refund
    Given an order "ORD-CANCEL-NO-PTS" with no loyalty points applied
    When order "ORD-CANCEL-NO-PTS" is cancelled with correlation "CORR-NO-PTS"
    Then within 5 seconds:
      | domain    | event_type          |
      | order     | OrderCancelled      |
      | inventory | ReservationReleased |
    And no PointsRefunded event is emitted for correlation "CORR-NO-PTS"

  # ===========================================================================
  # Correlation ID Preservation
  # ===========================================================================

  @e2e @saga @correlation
  Scenario: Correlation ID preserved through multi-hop saga chain
    # Order completion -> Fulfillment -> Shipping notification
    Given an order "ORD-CHAIN" ready for fulfillment
    When order "ORD-CHAIN" is completed with correlation "CHAIN-CORR"
    Then the correlation "CHAIN-CORR" appears in:
      | domain      | event_type        |
      | order       | OrderCompleted    |
      | fulfillment | ShipmentCreated   |
      | fulfillment | ShipmentPicked    |
      | fulfillment | ShipmentPacked    |
      | fulfillment | ShipmentShipped   |

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
  Scenario: Saga handles missing target aggregate gracefully
    Given a saga that targets non-existent aggregate "GHOST-AGG"
    When the triggering event occurs
    Then the saga command fails with sequence mismatch
    And the saga can retry with correct sequence

  @e2e @saga @errors
  Scenario: Saga idempotency on duplicate events
    Given a completed order "ORD-IDEM" that already triggered fulfillment
    When the OrderCompleted event is replayed
    Then no duplicate shipment is created
    And the saga recognizes already-processed event

  # ===========================================================================
  # Saga Timing and Ordering
  # ===========================================================================

  @e2e @saga @timing
  Scenario: Saga commands execute in order
    Given an order "ORD-ORDER" with multiple saga subscriptions
    When the order is completed
    Then saga commands are executed in registration order
    And no race conditions occur between sagas

  @e2e @saga @timing
  Scenario: Saga handles slow event processing
    Given network latency is simulated at 200ms
    When an order is completed
    Then the fulfillment saga eventually succeeds
    And correlation ID is preserved despite delays
