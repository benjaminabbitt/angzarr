Feature: DomainClient
  DomainClient combines QueryClient and AggregateClient into a single unified
  interface for interacting with a domain. This is the recommended entry point
  for most applications because:

  - **Single connection**: One endpoint, one channel, reduced resource usage
  - **Unified API**: Both queries and commands through one object
  - **Builder access**: Fluent builders attached to the client instance
  - **Simpler DI**: Inject one client instead of two

  DomainClient is a convenience wrapper. For advanced use cases (separate
  scaling, different endpoints), use QueryClient and AggregateClient directly.

  Background:
    Given a running aggregate coordinator for domain "test"
    And a registered aggregate handler for domain "test"

  Scenario: Connect to a domain and access both query and command APIs
    # DomainClient provides .query and .aggregate (or .command) accessors
    # for the underlying clients, enabling both read and write operations.
    When I create a DomainClient for the coordinator endpoint
    Then I should be able to query events
    And I should be able to send commands

  Scenario: Use command builder through domain client
    # The idiomatic pattern: client.command(domain, root).withX().execute()
    # This attaches the builder to the client for fluent usage.
    When I create a DomainClient for domain "test"
    And I use the command builder to send a command
    Then I should receive a CommandResponse

  Scenario: Use query builder through domain client
    # Similarly: client.query(domain, root).range(x).getPages()
    Given an aggregate "test" with root "550e8400-e29b-41d4-a716-446655440000" has 5 events
    When I create a DomainClient for domain "test"
    And I use the query builder to fetch events for that root
    Then I should receive 5 EventPages

  Scenario: Single connection serves both read and write
    # Verify that both operations use the same underlying channel.
    # This is an implementation detail but important for resource efficiency.
    When I create a DomainClient for the coordinator endpoint
    And I send a command
    And I query for the resulting events
    Then both operations should succeed on the same connection

  Scenario: Close domain client releases all resources
    # Closing DomainClient must close both underlying clients.
    # Resource leaks in long-running services cause gradual degradation.
    Given a connected DomainClient
    When I close the DomainClient
    Then subsequent commands should fail with ConnectionError
    And subsequent queries should fail with ConnectionError

  Scenario: Connect via environment variable
    # Consistent with other clients: production deployments use env vars.
    Given environment variable "TEST_DOMAIN_ENDPOINT" is set to the coordinator endpoint
    When I create a DomainClient from environment variable "TEST_DOMAIN_ENDPOINT"
    Then the DomainClient should be connected
