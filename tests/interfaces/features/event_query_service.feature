# docs:start:query_service_contract
Feature: EventQueryService interface
  The EventQueryService provides read access to event streams via gRPC. Clients
  use this service to reconstruct aggregate state, feed projections, perform
  audit queries, and trace correlated events across domains.

  Without this service, clients would need direct storage access, breaking the
  encapsulation that allows storage backend swaps and consistent API behavior
  across deployment modes.
# docs:end:query_service_contract

  Background:
    Given an EventQueryService backend

  # ==========================================================================
  # GetEventBook - Single Aggregate Query
  # ==========================================================================

  Scenario: Retrieve complete EventBook for an aggregate
    Given an aggregate "player" with 5 events
    When I query the EventBook for domain "player" and the aggregate root
    Then I should receive an EventBook with 5 events
    And events should be ordered by sequence ascending

  Scenario: Query non-existent aggregate returns empty EventBook
    Given no events exist for domain "player" and root "nonexistent-001"
    When I query the EventBook for domain "player" and root "nonexistent-001"
    Then I should receive an EventBook with 0 events

  Scenario: Query requires domain and root
    When I query the EventBook without a domain or root
    Then the query should fail with INVALID_ARGUMENT

  Scenario: Query with invalid UUID fails gracefully
    When I query the EventBook for domain "player" with an invalid root UUID
    Then the query should fail with INVALID_ARGUMENT

  # ==========================================================================
  # GetEventBook - Range Queries
  # ==========================================================================

  Scenario: Query with lower bound returns events from that sequence
    Given an aggregate "player" with 10 events
    When I query events from sequence 5
    Then I should receive an EventBook with 5 events
    And the first event should have sequence 5

  Scenario: Query with range returns bounded slice
    Given an aggregate "player" with 10 events
    When I query events from sequence 2 to 6
    Then I should receive an EventBook with 5 events
    And the first event should have sequence 2
    And the last event should have sequence 6

  Scenario: Query with range beyond history returns available events
    Given an aggregate "player" with 5 events
    When I query events from sequence 0 to 100
    Then I should receive an EventBook with 5 events

  Scenario: Query with start beyond history returns empty
    Given an aggregate "player" with 5 events
    When I query events from sequence 100
    Then I should receive an EventBook with 0 events

  # ==========================================================================
  # GetEventBook - Temporal Queries
  # ==========================================================================

  Scenario: Temporal query by sequence returns state at that point
    Given an aggregate "player" with events at sequences 0, 1, 2, 3, 4
    When I query as of sequence 2
    Then I should receive an EventBook with 3 events
    And the last event should have sequence 2

  Scenario: Temporal query by timestamp returns events up to that time
    Given an aggregate "player" with events at timestamps:
      | sequence | timestamp           |
      | 0        | 2024-01-01T00:00:00 |
      | 1        | 2024-01-02T00:00:00 |
      | 2        | 2024-01-03T00:00:00 |
    When I query as of timestamp "2024-01-02T00:00:00"
    Then I should receive an EventBook with 2 events
    And the last event should have sequence 1

  Scenario: Temporal query without point-in-time fails
    Given an aggregate "player" with 5 events
    When I query with temporal selection but no point-in-time
    Then the query should fail with INVALID_ARGUMENT

  # ==========================================================================
  # GetEventBook - Correlation ID Queries
  # ==========================================================================

  Scenario: Query by correlation ID returns matching events
    Given an aggregate "order" with correlation ID "workflow-123" and 3 events
    When I query by correlation ID "workflow-123"
    Then I should receive an EventBook with 3 events

  Scenario: Query by correlation ID returns empty for no matches
    When I query by correlation ID "nonexistent-correlation"
    Then I should receive an EventBook with 0 events

  Scenario: Correlation ID query does not require domain or root
    Given an aggregate "order" with correlation ID "workflow-456" and 2 events
    When I query by correlation ID "workflow-456" without domain or root
    Then I should receive an EventBook with 2 events

  # ==========================================================================
  # GetEventBook - Snapshot Handling
  # ==========================================================================

  Scenario: Event queries return all events regardless of snapshots
    Given an aggregate "customer" with 5 events and a snapshot at sequence 2
    When I query the EventBook for the aggregate
    Then I should receive an EventBook with 5 events
    And the EventBook should not include a snapshot

  # ==========================================================================
  # GetEvents - Streaming Query
  # ==========================================================================

  Scenario: Stream events for an aggregate
    Given an aggregate "player" with 5 events
    When I stream events for domain "player" and the aggregate root
    Then I should receive a stream with 1 EventBook
    And the EventBook should have 5 events

  Scenario: Stream events returns empty for non-existent aggregate
    When I stream events for domain "player" and root "nonexistent-001"
    Then I should receive a stream with 1 EventBook
    And the EventBook should have 0 events

  Scenario: Stream query requires domain and root
    When I stream events without a domain or root
    Then the stream should fail with INVALID_ARGUMENT

  # ==========================================================================
  # GetEvents - Correlation ID Streaming
  # ==========================================================================

  Scenario: Stream by correlation ID returns multiple EventBooks
    Given aggregates with correlation ID "multi-domain-workflow":
      | domain    | events |
      | order     | 2      |
      | inventory | 3      |
    When I stream events by correlation ID "multi-domain-workflow"
    Then I should receive a stream with 2 EventBooks
    And the total event count should be 5

  # ==========================================================================
  # GetAggregateRoots - Root Discovery
  # ==========================================================================

  Scenario: List all aggregate roots returns stream of roots
    Given aggregates in domain "player":
      | root       | events |
      | player-001 | 1      |
      | player-002 | 2      |
      | player-003 | 3      |
    When I list aggregate roots
    Then I should receive a stream with 3 roots
    And each root should include domain and UUID

  Scenario: List aggregate roots returns empty for no data
    When I list aggregate roots
    Then I should receive a stream with 0 roots

  Scenario: List aggregate roots shows roots across domains
    Given an aggregate "player" with 1 event
    And an aggregate "table" with 1 event
    And an aggregate "hand" with 1 event
    When I list aggregate roots
    Then I should receive a stream with 3 roots

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Malformed query returns clear error
    When I send a malformed query
    Then the query should fail with INVALID_ARGUMENT
    And the error message should describe the problem

  Scenario: Query for empty domain returns error
    When I query the EventBook for domain "" and root "some-root"
    Then the query should fail with INVALID_ARGUMENT
