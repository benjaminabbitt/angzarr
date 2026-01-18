@container
Feature: End-to-End Container Integration
  Verify the deployed angzarr system works correctly.

  Background:
    Given the angzarr system is running at "localhost:1350"

  @container
  Scenario: Create a customer via gateway
    Given a new customer id
    When I send a CreateCustomer command with name "Container Test" and email "container@test.com"
    Then the command succeeds
    And the latest event type is "CustomerCreated"
    And the customer aggregate has 1 event

  @container
  Scenario: Query customer events
    Given a new customer id
    And I send a CreateCustomer command with name "Query Test" and email "query@test.com"
    And the command succeeds
    When I query events for the customer aggregate
    Then I receive 1 event
    And the event at sequence 0 has type "CustomerCreated"

  @container
  Scenario: Synchronous projections returned in response
    Given a new customer id
    When I send a CreateCustomer command with name "Sync Projection Test" and email "sync@test.com"
    Then the command succeeds
    And the latest event type is "CustomerCreated"
    # Projections are returned synchronously when projector coordinators are configured
