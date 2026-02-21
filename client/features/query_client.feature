# docs:start:query_client_contract
Feature: QueryClient - Event Retrieval
  The QueryClient provides read access to aggregate event histories.
  It supports various query modes: full history, range queries, temporal
  queries, and correlation-based queries across aggregates.

  Without query access, clients cannot reconstruct aggregate state,
  catch up projectors, or debug event flows.
# docs:end:query_client_contract

  Background:
    Given a QueryClient connected to the test backend

  # ==========================================================================
  # Basic Event Retrieval
  # ==========================================================================

  # docs:start:client_query
  Scenario: Query returns empty for new aggregate
    Given an aggregate "orders" with root "order-new"
    When I query events for "orders" root "order-new"
    Then I should receive an EventBook with 0 events
    And the next_sequence should be 0

  Scenario: Query returns all events for existing aggregate
    Given an aggregate "orders" with root "order-001" has 5 events
    When I query events for "orders" root "order-001"
    Then I should receive an EventBook with 5 events
    And events should be in sequence order 0 to 4

  Scenario: Query preserves event payloads exactly
    Given an aggregate "orders" with root "order-002" has event "OrderCreated" with data "test-payload"
    When I query events for "orders" root "order-002"
    Then the first event should have type "OrderCreated"
    And the first event should have payload "test-payload"
  # docs:end:client_query

  # ==========================================================================
  # Range Queries
  # ==========================================================================

  Scenario: Range query from specific sequence
    Given an aggregate "orders" with root "order-003" has 10 events
    When I query events for "orders" root "order-003" from sequence 5
    Then I should receive an EventBook with 5 events
    And the first event should have sequence 5

  Scenario: Range query with upper bound
    Given an aggregate "orders" with root "order-004" has 10 events
    When I query events for "orders" root "order-004" from sequence 3 to 7
    Then I should receive an EventBook with 4 events
    And the first event should have sequence 3
    And the last event should have sequence 6

  Scenario: Range query beyond history returns empty
    Given an aggregate "orders" with root "order-005" has 5 events
    When I query events for "orders" root "order-005" from sequence 100
    Then I should receive an EventBook with 0 events

  # ==========================================================================
  # Temporal Queries
  # ==========================================================================

  Scenario: Query as of specific sequence
    Given an aggregate "orders" with root "order-006" has 10 events
    When I query events for "orders" root "order-006" as of sequence 5
    Then I should receive an EventBook with 6 events
    And the last event should have sequence 5

  Scenario: Query as of timestamp
    Given an aggregate "orders" with root "order-007" has events at known timestamps
    When I query events for "orders" root "order-007" as of time "2024-01-15T10:30:00Z"
    Then I should receive events up to that timestamp

  # ==========================================================================
  # Edition Queries
  # ==========================================================================

  Scenario: Query from specific edition
    Given an aggregate "orders" with root "order-008" in edition "test-branch"
    When I query events for "orders" root "order-008" in edition "test-branch"
    Then I should receive events from that edition only

  Scenario: Edition queries are isolated from main timeline
    Given an aggregate "orders" with root "order-009" has 3 events in main
    And an aggregate "orders" with root "order-009" has 2 events in edition "branch"
    When I query events for "orders" root "order-009"
    Then I should receive an EventBook with 3 events
    When I query events for "orders" root "order-009" in edition "branch"
    Then I should receive an EventBook with 2 events

  # ==========================================================================
  # Correlation ID Queries
  # ==========================================================================

  Scenario: Query by correlation ID across aggregates
    Given events with correlation ID "workflow-123" exist in multiple aggregates
    When I query events by correlation ID "workflow-123"
    Then I should receive events from all correlated aggregates

  Scenario: Unknown correlation ID returns empty
    When I query events by correlation ID "nonexistent-correlation"
    Then I should receive no events

  # ==========================================================================
  # Snapshot Integration
  # ==========================================================================

  Scenario: Query includes snapshot when available
    Given an aggregate "orders" with root "order-010" has a snapshot at sequence 5 and 10 events
    When I query events for "orders" root "order-010"
    Then the EventBook should include the snapshot
    And the snapshot should be at sequence 5

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Query with invalid domain returns error
    When I query events with empty domain
    Then the operation should fail with invalid argument error

  Scenario: Query handles connection failure gracefully
    Given the query service is unavailable
    When I attempt to query events
    Then the operation should fail with connection error
