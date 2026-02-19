Feature: QueryClient
  The QueryClient provides read access to aggregate event streams. In event-sourced
  systems, all state is derived from events. QueryClient enables:

  - **State reconstruction**: Fetch events to rebuild aggregate state locally
  - **Audit trails**: Read complete history for debugging and compliance
  - **Projections**: Feed events to read-model projectors
  - **Testing**: Verify events were persisted correctly after commands

  Every language SDK must provide a QueryClient with identical capabilities to ensure
  developers can switch languages without relearning the API.

  Background:
    Given a running aggregate coordinator for domain "test"

  Scenario: Connect and retrieve events for an aggregate
    # The fundamental operation: fetch all events for a specific aggregate instance.
    # Without this, clients cannot reconstruct aggregate state or verify command results.
    Given an aggregate "test" with root "550e8400-e29b-41d4-a716-446655440000" has 3 events
    When I create a QueryClient for the coordinator endpoint
    And I query events for domain "test" and root "550e8400-e29b-41d4-a716-446655440000"
    Then I should receive an EventBook with 3 pages

  Scenario: Connect via environment variable for deployment flexibility
    # Production deployments use environment variables for configuration.
    # This enables the same binary to run in different environments (dev/staging/prod)
    # without code changes. The SDK must support this pattern natively.
    Given environment variable "TEST_QUERY_ENDPOINT" is set to the coordinator endpoint
    When I create a QueryClient from environment variable "TEST_QUERY_ENDPOINT"
    Then the QueryClient should be connected

  Scenario: Handle connection errors gracefully with typed exceptions
    # Network failures are inevitable. The SDK must surface connection errors
    # as typed exceptions so callers can implement retry logic, circuit breakers,
    # or graceful degradation. Swallowing errors or using generic exceptions
    # makes debugging production issues nearly impossible.
    When I create a QueryClient for endpoint "localhost:99999"
    And I attempt to query events
    Then I should receive a ConnectionError

  Scenario: Close connection to release resources
    # gRPC connections hold system resources (file descriptors, memory).
    # Long-running processes that create many clients will leak resources
    # without explicit cleanup. The SDK must provide deterministic disposal.
    Given a connected QueryClient
    When I close the QueryClient
    Then subsequent queries should fail with ConnectionError
