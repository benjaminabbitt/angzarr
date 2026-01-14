@container
Feature: End-to-End Integration
  As an event-sourced system
  I want to process commands through business logic, sagas, and projectors
  So that the full event-driven workflow functions correctly

  # All commands go through angzarr's BusinessCoordinator service.
  # Angzarr routes to the appropriate business logic, projectors, and sagas.
  # The gateway provides streaming command/event support.

  Background:
    Given the angzarr system is running at "localhost:50051"
    And the streaming gateway is running at "localhost:50053"

  # Direct command scenarios

  Scenario: Create a new customer
    Given a new customer id
    When I send a CreateCustomer command with name "Alice" and email "alice@example.com"
    Then the command succeeds
    And the customer aggregate has 1 event
    And the latest event type is "CustomerCreated"

  Scenario: Create and complete a transaction generates receipt projection
    Given a new customer id
    And I send a CreateCustomer command with name "Bob" and email "bob@example.com"
    And the command succeeds
    Given a new transaction id for the customer
    When I send a CreateTransaction command with items:
      | product_id | name       | quantity | unit_price_cents |
      | SKU-001    | Widget     | 2        | 1000             |
      | SKU-002    | Gadget     | 1        | 2500             |
    Then the command succeeds
    And the transaction aggregate has 1 event
    When I send a CompleteTransaction command with payment method "card"
    Then the command succeeds
    And the transaction aggregate has 2 events
    And the latest event type is "TransactionCompleted"
    And a projection was returned from projector "receipt"
    And the projection contains a Receipt with total 4500 cents

  Scenario: Query events for an aggregate
    Given a new customer id
    And I send a CreateCustomer command with name "Charlie" and email "charlie@example.com"
    When I query events for the customer aggregate
    Then I receive 1 event
    And the event at sequence 0 has type "CustomerCreated"

  # Gateway streaming scenarios

  Scenario: Send command via gateway and receive events
    Given a new customer id
    When I send a CreateCustomer command via gateway with name "StreamTest" and email "stream@test.com"
    Then I receive at least 1 event from the stream
    And the streamed events include type "CustomerCreated"

  Scenario: Correlation ID is preserved across events
    Given a new customer id
    When I send a CreateCustomer command via gateway with name "CorrelationTest" and email "corr@test.com"
    Then all streamed events have the same correlation ID

  Scenario: Multiple commands with different correlation IDs are isolated
    Given a new customer id as "customer1"
    And a new customer id as "customer2"
    When I send a CreateCustomer command via gateway for "customer1" with name "First" and email "first@test.com"
    And I send a CreateCustomer command via gateway for "customer2" with name "Second" and email "second@test.com"
    Then events for "customer1" only contain "First"
    And events for "customer2" only contain "Second"

  Scenario: Stream timeout returns when no events arrive
    Given a new customer id
    When I subscribe to events with a non-matching correlation ID
    Then the stream closes after the timeout period

  Scenario: Transaction flow streams all related events
    Given a new customer id
    And I send a CreateCustomer command via gateway with name "TxTest" and email "tx@test.com"
    Given a new transaction id for the customer
    When I send a CreateTransaction command via gateway with items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-100    | Item A | 1        | 500              |
    Then I receive at least 1 event from the stream
    When I send a CompleteTransaction command via gateway with payment method "cash"
    Then I receive at least 1 event from the stream
    And the streamed events include type "TransactionCompleted"
