Feature: SnapshotStore interface
  Snapshots are a performance optimization for aggregates with long histories.
  Loading an aggregate means: fetch snapshot + replay events after snapshot.
  Without snapshots, every load replays from event 0.

  Background:
    Given a SnapshotStore backend

  # ==========================================================================
  # Snapshot Retrieval
  # ==========================================================================

  Scenario: New aggregates have no snapshot - full replay required
    Given an aggregate "player" with no snapshot
    When I get the snapshot for the aggregate
    Then the snapshot should not exist

  Scenario: Snapshotted aggregates load quickly via checkpoint
    Given an aggregate "player" with a snapshot at sequence 10
    When I get the snapshot for the aggregate
    Then the snapshot should exist
    And the snapshot should have sequence 10

  Scenario: Snapshot state bytes are preserved exactly
    Given an aggregate "player" with no snapshot
    When I put a snapshot at sequence 5 with data "serialized-player-state"
    And I get the snapshot for the aggregate
    Then the snapshot should have data "serialized-player-state"

  # ==========================================================================
  # Snapshot Updates
  # ==========================================================================

  Scenario: First snapshot establishes the initial checkpoint
    Given an aggregate "player" with no snapshot
    When I put a snapshot at sequence 5
    And I get the snapshot for the aggregate
    Then the snapshot should exist
    And the snapshot should have sequence 5

  Scenario: New snapshots replace old ones atomically
    Given an aggregate "player" with a snapshot at sequence 5
    When I put a snapshot at sequence 15
    And I get the snapshot for the aggregate
    Then the snapshot should have sequence 15

  Scenario: High-frequency snapshots don't accumulate
    Given an aggregate "player" with no snapshot
    When I put a snapshot at sequence 1
    And I put a snapshot at sequence 5
    And I put a snapshot at sequence 10
    And I put a snapshot at sequence 20
    And I put a snapshot at sequence 50
    And I get the snapshot for the aggregate
    Then the snapshot should have sequence 50

  # ==========================================================================
  # Snapshot Deletion
  # ==========================================================================

  Scenario: Schema changes require snapshot invalidation
    Given an aggregate "player" with a snapshot at sequence 10
    When I delete the snapshot for the aggregate
    And I get the snapshot for the aggregate
    Then the snapshot should not exist

  Scenario: Bulk deletion doesn't require existence checks
    Given an aggregate "player" with no snapshot
    When I delete the snapshot for the aggregate
    Then the operation should succeed

  Scenario: Deleted snapshots don't prevent future snapshotting
    Given an aggregate "player" with a snapshot at sequence 5
    When I delete the snapshot for the aggregate
    And I put a snapshot at sequence 20
    And I get the snapshot for the aggregate
    Then the snapshot should have sequence 20

  # ==========================================================================
  # Aggregate Isolation
  # ==========================================================================

  Scenario: Each aggregate root has its own independent snapshot
    Given an aggregate "player" with root "player-001" and a snapshot at sequence 10
    And an aggregate "player" with root "player-002" and a snapshot at sequence 20
    When I get the snapshot for root "player-001" in domain "player"
    Then the snapshot should have sequence 10
    When I get the snapshot for root "player-002" in domain "player"
    Then the snapshot should have sequence 20

  Scenario: Snapshot operations on one aggregate don't affect others
    Given an aggregate "player" with root "player-001" and a snapshot at sequence 10
    And an aggregate "player" with root "player-002" and a snapshot at sequence 20
    When I delete the snapshot for root "player-001" in domain "player"
    Then the snapshot for root "player-001" should not exist
    And the snapshot for root "player-002" should exist

  # ==========================================================================
  # Domain Isolation
  # ==========================================================================

  Scenario: Bounded contexts maintain snapshot isolation
    Given an aggregate "player" with a snapshot at sequence 10
    And an aggregate "table" with a snapshot at sequence 20
    When I get the snapshot for domain "player"
    Then the snapshot should have sequence 10
    When I get the snapshot for domain "table"
    Then the snapshot should have sequence 20
