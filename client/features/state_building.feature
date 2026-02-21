# docs:start:state_building_contract
Feature: State Building - Aggregate State Reconstruction
  Aggregates reconstruct their state by replaying events. This feature
  tests the state building process including snapshot integration,
  event application, and the build_state pattern used across languages.

  State building is the foundation of event sourcing - without correct
  state reconstruction, commands cannot make valid decisions.
# docs:end:state_building_contract

  # ==========================================================================
  # Basic State Building
  # ==========================================================================

  Scenario: Build state from empty event history
    Given an aggregate type with default state
    And an empty EventBook
    When I build state from the EventBook
    Then the state should be the default state
    And no events should have been applied

  Scenario: Build state from single event
    Given an aggregate type with default state
    And an EventBook with 1 event of type "OrderCreated"
    When I build state from the EventBook
    Then the state should reflect the OrderCreated event
    And the state should have order_id set

  Scenario: Build state from multiple events
    Given an aggregate type with default state
    And an EventBook with events:
      | sequence | type        |
      | 0        | OrderCreated|
      | 1        | ItemAdded   |
      | 2        | ItemAdded   |
    When I build state from the EventBook
    Then the state should reflect all 3 events
    And the state should have 2 items

  Scenario: Events are applied in sequence order
    Given an EventBook with events in order: A, B, C
    When I build state
    Then events should be applied as A, then B, then C
    And final state should reflect the correct order

  # ==========================================================================
  # Snapshot Integration
  # ==========================================================================

  Scenario: Build state from snapshot only
    Given an EventBook with a snapshot at sequence 5
    And no events in the EventBook
    When I build state from the EventBook
    Then the state should equal the snapshot state
    And no events should be applied

  Scenario: Build state from snapshot plus events
    Given an EventBook with:
      | snapshot_sequence | 5                |
      | events            | seq 6, 7, 8, 9   |
    When I build state from the EventBook
    Then the state should start from snapshot
    And only events 6, 7, 8, 9 should be applied

  Scenario: Events before snapshot are ignored
    Given an EventBook with:
      | snapshot_sequence | 5           |
      | events            | seq 3, 4, 6, 7 |
    When I build state from the EventBook
    Then events at seq 3 and 4 should NOT be applied
    And only events at seq 6 and 7 should be applied

  # ==========================================================================
  # Event Application
  # ==========================================================================

  Scenario: Unknown event types are skipped
    Given an EventBook with an event of unknown type
    When I build state from the EventBook
    Then the unknown event should be skipped
    And no error should occur
    And other events should still be applied

  Scenario: Event application modifies state
    Given initial state with field value 0
    And an event that increments field by 10
    When I apply the event to state
    Then the field should equal 10

  Scenario: Cumulative event application
    Given initial state with field value 0
    And events that increment by 5, 3, and 2
    When I apply all events to state
    Then the field should equal 10

  # ==========================================================================
  # Any-Wrapped Events
  # ==========================================================================

  Scenario: Build state handles Any-wrapped events
    Given events wrapped in google.protobuf.Any
    When I build state from the EventBook
    Then the Any wrapper should be unpacked
    And the typed event should be applied

  Scenario: Type URL determines event type
    Given an event with type_url "type.googleapis.com/orders.ItemAdded"
    When I apply the event
    Then the ItemAdded handler should be invoked
    And the type_url suffix should match the handler

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Malformed event payload causes error
    Given an event with corrupted payload bytes
    When I attempt to build state
    Then an error should be raised
    And the error should indicate deserialization failure

  Scenario: Missing required field in event
    Given an event missing a required field
    When I attempt to build state
    Then the behavior depends on language
    And either default value is used or error is raised

  # ==========================================================================
  # Next Sequence Calculation
  # ==========================================================================

  Scenario: Next sequence from empty aggregate
    Given an EventBook with no events and no snapshot
    When I get next_sequence
    Then next_sequence should be 0

  Scenario: Next sequence from events
    Given an EventBook with events up to sequence 4
    When I get next_sequence
    Then next_sequence should be 5

  Scenario: Next sequence from snapshot only
    Given an EventBook with snapshot at sequence 10 and no events
    When I get next_sequence
    Then next_sequence should be 11

  Scenario: Next sequence from snapshot plus events
    Given an EventBook with snapshot at 5 and events up to 8
    When I get next_sequence
    Then next_sequence should be 9

  # ==========================================================================
  # Immutability
  # ==========================================================================

  Scenario: State building does not modify EventBook
    Given an EventBook
    When I build state from the EventBook
    Then the EventBook should be unchanged
    And the EventBook events should still be present

  Scenario: State building returns new state object
    Given an existing state object
    When I build state from events
    Then a new state object should be returned
    And the original state should be unchanged

  # ==========================================================================
  # Language-Specific Patterns
  # ==========================================================================

  Scenario: build_state takes state and Any-wrapped events
    Given a build_state function
    When I call build_state(state, events)
    Then each event should be unpacked from Any
    And _apply_event should be called for each
    And final state should be returned

  Scenario: _apply_event dispatches by type
    Given an _apply_event function
    When I call _apply_event(state, event_any)
    Then the event should be unpacked
    And the correct type handler should be invoked
    And state should be mutated
