Feature: Fulfillment Saga
  Tests the saga that creates shipments when payment is confirmed.
  Listens to OrderCompleted events and generates CreateShipment commands.

  Scenario: Create shipment when order completes
    Given an OrderCompleted event for order "ORD-001"
    When I process the fulfillment saga
    Then a CreateShipment command is generated
    And the command targets "fulfillment" domain
    And the command has order_id "ORD-001"

  Scenario: Ignore OrderCreated events
    Given an OrderCreated event for order "ORD-002"
    When I process the fulfillment saga
    Then no commands are generated

  Scenario: Ignore OrderCancelled events
    Given an OrderCancelled event for order "ORD-003"
    When I process the fulfillment saga
    Then no commands are generated

  Scenario: Preserve correlation ID
    Given an OrderCompleted event for order "ORD-004"
    And the correlation_id is "CORR-FULFILL-001"
    When I process the fulfillment saga
    Then the command has correlation_id "CORR-FULFILL-001"
