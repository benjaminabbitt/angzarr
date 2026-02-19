Feature: QueryBuilder
  QueryBuilder provides a fluent API for constructing event queries. Event queries
  support multiple access patterns:

  - **By root**: Fetch all events for a specific aggregate instance
  - **By correlation ID**: Fetch events across aggregates sharing a workflow
  - **By sequence range**: Fetch specific event windows for pagination or replay
  - **By temporal point**: Reconstruct historical state (as-of queries)
  - **By edition**: Query from specific schema versions after upcasting

  These patterns enable state reconstruction, auditing, debugging, and projection.
  The builder abstracts proto complexity and provides type-safe construction.

  Background:
    Given a QueryClient connected to the coordinator

  Scenario: Build basic query by domain and root
    # The fundamental query: fetch all events for one aggregate instance.
    # Every other query pattern builds on this foundation.
    When I build a query using QueryBuilder:
      | field  | value                                |
      | domain | test                                 |
      | root   | 550e8400-e29b-41d4-a716-446655440000 |
    Then the resulting Query should have:
      | field  | value                                |
      | domain | test                                 |
      | root   | 550e8400-e29b-41d4-a716-446655440000 |

  Scenario: Query with sequence range for pagination
    # Large aggregates may have thousands of events. Sequence ranges enable:
    # - Pagination: fetch events 100-200, then 200-300
    # - Incremental sync: "give me events since sequence 500"
    # - Bounded replay: replay only recent events for faster startup
    When I build a query with range from 5 to 10
    Then the resulting Query should have sequence_range with lower=5 and upper=10

  Scenario: Query with open-ended range (lower bound only)
    # "Give me all events from sequence N onwards" is essential for:
    # - Catching up after disconnection
    # - Incremental projection updates
    # - Event streaming from a known checkpoint
    When I build a query with range from 5
    Then the resulting Query should have sequence_range with lower=5 and no upper bound

  Scenario: Temporal query as_of_sequence for point-in-time state
    # Reconstruct aggregate state as it existed at sequence N.
    # Essential for:
    # - Debugging: "What was the state when this bug occurred?"
    # - Compliance: "What was the account balance on this date?"
    # - Replay: Rebuild read models from historical state
    When I build a query as_of_sequence 42
    Then the resulting Query should have temporal_query with sequence=42

  Scenario: Temporal query as_of_time for timestamp-based reconstruction
    # RFC3339 timestamps enable human-readable temporal queries.
    # "Show me the inventory state at 2024-01-15T10:30:00Z"
    # The SDK must parse and convert to protobuf Timestamp correctly.
    When I build a query as_of_time "2024-01-15T10:30:00Z"
    Then the resulting Query should have temporal_query with the parsed timestamp

  Scenario: Query by correlation ID for cross-aggregate workflows
    # Correlation IDs link events across aggregates in a distributed workflow.
    # "Show me all events for order workflow corr-456" returns events from
    # order, inventory, fulfillment, and payment aggregates.
    # Essential for debugging saga/process manager flows.
    When I build a query by_correlation_id "corr-456"
    Then the resulting Query should query by correlation_id "corr-456"

  Scenario: Query with edition filter for schema-versioned events
    # After upcasting (event schema migration), events exist in multiple editions.
    # Edition filtering enables:
    # - Forward compatibility: query only events your code understands
    # - Migration validation: compare events before/after upcasting
    When I build a query with_edition "v2"
    Then the resulting Query should have edition "v2"

  Scenario: Execute query and return full EventBook
    # EventBook includes Cover (metadata) and Pages (events).
    # The Cover contains domain, root, and correlation ID for context.
    Given an aggregate "test" with root "550e8400-e29b-41d4-a716-446655440000" has 5 events
    When I use QueryBuilder to get_event_book for that root
    Then I should receive an EventBook with 5 pages
    And the EventBook should have the correct domain and root

  Scenario: Execute query and return only pages for convenience
    # When you only need events (not metadata), get_pages() reduces boilerplate.
    # Common for state reconstruction where Cover is already known.
    Given an aggregate "test" with root "550e8400-e29b-41d4-a716-446655440000" has 5 events
    When I use QueryBuilder to get_pages for that root
    Then I should receive a list of 5 EventPages
