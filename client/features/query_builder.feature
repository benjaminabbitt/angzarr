# docs:start:query_builder_contract
Feature: QueryBuilder - Fluent Query Construction
  The QueryBuilder provides a fluent API for constructing event queries.
  It supports various selection modes: ranges, temporal queries, and
  correlation-based queries. Handles edition selection and pagination.

  The builder pattern enables both OO-style client usage and can be
  adapted for router-based implementations.
# docs:end:query_builder_contract

  Background:
    Given a mock QueryClient for testing

  # ==========================================================================
  # Basic Query Construction
  # ==========================================================================

  Scenario: Build query with domain and root
    When I build a query for domain "orders" root "order-001"
    Then the built query should have domain "orders"
    And the built query should have root "order-001"

  Scenario: Build query for domain only
    When I build a query for domain "orders" without root
    Then the built query should have domain "orders"
    And the built query should have no root

  # ==========================================================================
  # Range Selection
  # ==========================================================================

  Scenario: Build query with lower bound range
    When I build a query for domain "orders" root "order-002"
      And I set range from 10
    Then the built query should have range selection
    And the range lower bound should be 10
    And the range upper bound should be empty

  Scenario: Build query with bounded range
    When I build a query for domain "orders" root "order-003"
      And I set range from 5 to 15
    Then the built query should have range selection
    And the range lower bound should be 5
    And the range upper bound should be 15

  # ==========================================================================
  # Temporal Selection
  # ==========================================================================

  Scenario: Build query as of sequence
    When I build a query for domain "orders" root "order-004"
      And I set as_of_sequence to 42
    Then the built query should have temporal selection
    And the point_in_time should be sequence 42

  Scenario: Build query as of timestamp
    When I build a query for domain "orders" root "order-005"
      And I set as_of_time to "2024-01-15T10:30:00Z"
    Then the built query should have temporal selection
    And the point_in_time should be the parsed timestamp

  Scenario: Build query with invalid timestamp fails
    When I build a query for domain "orders"
      And I set as_of_time to "not-a-timestamp"
    Then building should fail
    And the error should indicate invalid timestamp

  # ==========================================================================
  # Correlation ID Queries
  # ==========================================================================

  Scenario: Build query by correlation ID
    When I build a query for domain "orders"
      And I set by_correlation_id to "workflow-123"
    Then the built query should have correlation ID "workflow-123"
    And the built query should have no root

  Scenario: Correlation ID clears root
    When I build a query for domain "orders" root "order-006"
      And I set by_correlation_id to "workflow-456"
    Then the built query should have no root
    And the built query should have correlation ID "workflow-456"

  # ==========================================================================
  # Edition Selection
  # ==========================================================================

  Scenario: Build query for specific edition
    When I build a query for domain "orders" root "order-007"
      And I set edition to "test-branch"
    Then the built query should have edition "test-branch"

  Scenario: Build query without edition uses main timeline
    When I build a query for domain "orders" root "order-008"
    Then the built query should have no edition
    And the query should target main timeline

  # ==========================================================================
  # Fluent Chaining
  # ==========================================================================

  Scenario: Builder methods can be chained
    When I build a query using fluent chaining:
      """
      client.query("orders", root)
        .edition("test-branch")
        .range(10)
        .build()
      """
    Then the build should succeed
    And all chained values should be preserved

  Scenario: Last selection wins
    When I build a query with:
      """
      client.query("orders", root)
        .range(5)
        .as_of_sequence(10)
      """
    Then the query should have temporal selection (last set)
    And the range selection should be replaced

  # ==========================================================================
  # Execute Integration
  # ==========================================================================

  Scenario: Builder get_events executes and returns EventBook
    When I build and get_events for domain "orders" root "order-009"
    Then the query should be sent to the query service
    And an EventBook should be returned

  Scenario: Builder get_pages returns just the pages
    When I build and get_pages for domain "orders" root "order-010"
    Then only the event pages should be returned
    And the EventBook metadata should be stripped

  # ==========================================================================
  # Extension Traits
  # ==========================================================================

  Scenario: Client provides query builder shortcut
    Given a QueryClient implementation
    When I call client.query("orders", root)
    Then I should receive a QueryBuilder for that domain and root

  Scenario: Client provides query_domain shortcut
    Given a QueryClient implementation
    When I call client.query_domain("orders")
    Then I should receive a QueryBuilder with no root set
    And I can chain by_correlation_id
