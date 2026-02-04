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
  # Correlation ID Preservation
  # ===========================================================================

  @e2e @saga @correlation
  Scenario: Different orders maintain separate correlation IDs
    When I create order "ORD-A" with correlation "CORR-A"
    And I create order "ORD-B" with correlation "CORR-B"
    And both orders are completed
    Then fulfillment events for "ORD-A" have correlation "CORR-A"
    And fulfillment events for "ORD-B" have correlation "CORR-B"
    And no cross-contamination of correlation IDs
