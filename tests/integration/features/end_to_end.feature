Feature: End-to-End Integration
  As an event-sourced system
  I want to process commands through business logic, sagas, and projectors
  So that the full event-driven workflow functions correctly

  # All commands go through evented's BusinessCoordinator service.
  # Evented routes to the appropriate business logic, projectors, and sagas.

  Background:
    Given the evented system is running at "localhost:50051"

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
