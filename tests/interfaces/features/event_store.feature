Feature: EventStore interface
  The EventStore is the source of truth for all state changes in the system.
  Every aggregate's current state is derived by replaying its events. This
  immutability provides a complete audit trail, enables temporal queries, and
  allows the system to reconstruct any aggregate's state at any point in history.

  Background:
    Given an EventStore backend

  # ==========================================================================
  # Appending Events
  # ==========================================================================

  Scenario: First event in an aggregate's history starts at sequence 0
    Given an aggregate "player" with no events
    When I add 1 event to the aggregate
    Then the aggregate should have 1 event
    And the first event should have sequence 0

  Scenario: Multiple events from a single command receive consecutive sequences
    Given an aggregate "player" with no events
    When I add 5 events to the aggregate
    Then the aggregate should have 5 events
    And events should have consecutive sequences starting from 0

  Scenario: Commands that produce no events leave state unchanged
    Given an aggregate "player" with no events
    Then the aggregate should have 0 events

  Scenario: Each command batch continues from the previous sequence
    Given an aggregate "player" with no events
    When I add 2 events to the aggregate
    And I add 3 events to the aggregate
    Then the aggregate should have 5 events
    And events should have consecutive sequences starting from 0

  # ==========================================================================
  # Optimistic Concurrency Control
  # ==========================================================================

  Scenario: Concurrent writers are detected via sequence mismatch
    Given an aggregate "player" with 3 events
    When I try to add an event with sequence 1
    Then the operation should fail with a sequence conflict

  Scenario: Stale writers cannot overwrite history
    Given an aggregate "player" with 3 events
    When I try to add an event with sequence 0
    Then the operation should fail with a sequence conflict

  # ==========================================================================
  # Event Retrieval for State Reconstruction
  # ==========================================================================

  Scenario: Loading an aggregate replays its complete history in order
    Given an aggregate "player" with 10 events
    When I get all events from the aggregate
    Then I should receive 10 events
    And events should be ordered by sequence ascending

  Scenario: New aggregates have no history to replay
    Given an aggregate "player" with no events
    When I get all events from the aggregate
    Then I should receive 0 events

  Scenario: Event payloads are preserved exactly through storage
    Given an aggregate "player" with no events
    When I add an event with type "PlayerRegistered" and payload "alice@example.com"
    And I get all events from the aggregate
    Then the first event should have type "PlayerRegistered"
    And the first event should have payload "alice@example.com"

  # ==========================================================================
  # Partial Event Retrieval
  # ==========================================================================

  Scenario: Snapshot optimization - only replay events after the snapshot
    Given an aggregate "player" with 10 events
    When I get events from sequence 5
    Then I should receive 5 events
    And the first event should have sequence 5

  Scenario: Projectors can poll for just the latest event
    Given an aggregate "player" with 5 events
    When I get events from sequence 4
    Then I should receive 1 event

  Scenario: Polling for new events returns empty when caught up
    Given an aggregate "player" with 5 events
    When I get events from sequence 100
    Then I should receive 0 events

  Scenario: Audit queries can request a specific slice of history
    Given an aggregate "player" with 10 events
    When I get events from sequence 3 to 7
    Then I should receive 4 events
    And the first event should have sequence 3
    And the last event should have sequence 6

  # ==========================================================================
  # Aggregate Root Discovery
  # ==========================================================================

  Scenario: Operations can enumerate all aggregates in a domain
    Given 3 aggregates in domain "player" each with 1 event
    When I list roots for domain "player"
    Then I should see 3 roots in the list

  Scenario: Unused domains return an empty root list
    When I list roots for domain "unused_domain"
    Then I should see 0 roots in the list

  Scenario: Bounded contexts maintain strict isolation
    Given an aggregate "player" with root "player-001" and 1 events
    And an aggregate "table" with root "table-001" and 1 events
    When I list roots for domain "player"
    Then I should see 1 root in the list
    And the root should not appear in domain "table"

  # ==========================================================================
  # Domain Discovery
  # ==========================================================================

  Scenario: System inventory shows all active bounded contexts
    Given an aggregate "player" with 1 event
    And an aggregate "table" with 1 event
    And an aggregate "hand" with 1 event
    When I list all domains
    Then the domain list should contain "player"
    And the domain list should contain "table"
    And the domain list should contain "hand"

  # ==========================================================================
  # Next Sequence Calculation
  # ==========================================================================

  Scenario: New aggregates begin their sequence at zero
    Given an aggregate "player" with no events
    When I get the next sequence for the aggregate
    Then the next sequence should be 0

  Scenario: Next sequence reflects the aggregate's current length
    Given an aggregate "player" with 7 events
    When I get the next sequence for the aggregate
    Then the next sequence should be 7

  Scenario: Sequence advances atomically with each write
    Given an aggregate "player" with no events
    When I get the next sequence for the aggregate
    Then the next sequence should be 0
    When I add 1 event to the aggregate
    And I get the next sequence for the aggregate
    Then the next sequence should be 1
    When I add 3 events to the aggregate
    And I get the next sequence for the aggregate
    Then the next sequence should be 4

  # ==========================================================================
  # Aggregate Isolation
  # ==========================================================================

  Scenario: Each aggregate root maintains its own independent event stream
    Given an aggregate "player" with root "player-001" and 3 events
    And an aggregate "player" with root "player-002" and 5 events
    When I get events for root "player-001" in domain "player"
    Then I should receive 3 events
    When I get events for root "player-002" in domain "player"
    Then I should receive 5 events

  # ==========================================================================
  # Scale Testing
  # ==========================================================================

  Scenario: Long-lived aggregates with extensive history remain correct
    Given an aggregate "account" with no events
    When I add 100 events to the aggregate
    Then the aggregate should have 100 events
    And events should have consecutive sequences starting from 0
