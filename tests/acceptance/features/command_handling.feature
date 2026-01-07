Feature: Command Handling
  As an event-sourced application
  I want to handle commands through the business coordinator
  So that events are persisted and distributed

  Background:
    Given an empty event store
    And a stub business logic service

  Scenario: Handle command for new aggregate
    Given no prior events for aggregate "test-123" in domain "orders"
    When I send a "CreateOrder" command for aggregate "test-123"
    Then the business logic receives the command with empty event history
    And 1 event is persisted for aggregate "test-123"
    And the event bus receives the new events

  Scenario: Handle command with existing history
    Given prior events for aggregate "test-456" in domain "orders":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
    When I send an "AddItem" command for aggregate "test-456"
    Then the business logic receives the command with 2 prior events
    And 3 events total exist for aggregate "test-456"

  Scenario: Handle command with snapshot optimization
    Given prior events for aggregate "test-789" in domain "orders":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
      | 2        | ItemAdded    |
    And a snapshot at sequence 2 for aggregate "test-789"
    When I send an "AddItem" command for aggregate "test-789"
    Then the business logic receives the snapshot and events from sequence 2

  Scenario: Record events directly (saga use case)
    Given no prior events for aggregate "saga-001" in domain "sagas"
    When I record events directly for aggregate "saga-001":
      | sequence | event_type     |
      | 0        | SagaStarted    |
      | 1        | StepCompleted  |
    Then 2 events are persisted for aggregate "saga-001"
    And the event bus receives the new events
