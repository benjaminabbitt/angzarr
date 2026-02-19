Feature: AggregateClient
  The AggregateClient sends commands to aggregates through the coordinator.
  Commands are the only way to modify aggregate state in an event-sourced system.
  The client handles:

  - **Command routing**: Delivers commands to the correct aggregate coordinator
  - **Response handling**: Parses success/rejection responses
  - **Synchronization modes**: Async (fire-and-forget) vs sync (wait for persistence)
  - **Speculative execution**: What-if scenarios without persisting

  Consistent command APIs across languages ensure polyglot teams can contribute
  to any service without SDK-specific training.

  Background:
    Given a running aggregate coordinator for domain "test"
    And a registered aggregate handler for domain "test"

  Scenario: Send command asynchronously (fire-and-forget)
    # Async commands return immediately after the coordinator accepts them.
    # Use this for high-throughput scenarios where eventual consistency is acceptable.
    # The command is guaranteed to be processed, but the client doesn't wait.
    When I create an AggregateClient for the coordinator endpoint
    And I send a command to domain "test" with a new root
    Then I should receive a CommandResponse indicating acceptance

  Scenario: Send synchronous command and receive resulting events
    # Sync commands block until the aggregate processes the command and events
    # are persisted. The response includes the resulting events, enabling the
    # client to immediately see the effect of its command without a separate query.
    # Essential for UIs that need to display updated state immediately.
    When I create an AggregateClient for the coordinator endpoint
    And I send a synchronous command to domain "test"
    Then I should receive a CommandResponse containing the resulting events

  Scenario: Handle command rejection with business-meaningful errors
    # Aggregates reject commands that violate business rules (insufficient funds,
    # item out of stock, invalid state transition). The SDK must surface these
    # rejections with the aggregate's rejection reason intact, not as generic errors.
    # This enables clients to display user-friendly error messages.
    Given an aggregate handler that rejects commands with reason "invalid state"
    When I create an AggregateClient for the coordinator endpoint
    And I send a command that will be rejected
    Then I should receive a CommandResponse with rejection reason "invalid state"

  Scenario: Connect via environment variable for deployment flexibility
    # Same rationale as QueryClient: production deployments require external
    # configuration. Services discover coordinator endpoints through environment
    # variables set by the deployment platform (Kubernetes, Docker, etc.).
    Given environment variable "TEST_AGG_ENDPOINT" is set to the coordinator endpoint
    When I create an AggregateClient from environment variable "TEST_AGG_ENDPOINT"
    Then the AggregateClient should be connected

  Scenario: Speculative execution for what-if validation
    # Speculative mode runs the command against temporal state without persisting.
    # Use cases:
    # - Form validation: "Will this order succeed?" before user commits
    # - Preview: "What events would this produce?" for debugging
    # - Testing: Verify business logic without polluting event store
    # The aggregate state must remain unchanged after speculative execution.
    Given an aggregate "test" with root "550e8400-e29b-41d4-a716-446655440000" has 3 events
    When I create an AggregateClient for the coordinator endpoint
    And I send a speculative command to that aggregate
    Then I should receive a CommandResponse with projected events
    And the aggregate should still have only 3 persisted events

  Scenario: Handle connection errors with typed exceptions
    # Network failures between client and coordinator must be distinguishable
    # from business rejections. ConnectionError enables retry logic;
    # CommandRejectedError indicates the command reached the aggregate but failed validation.
    When I create an AggregateClient for endpoint "localhost:99999"
    And I attempt to send a command
    Then I should receive a ConnectionError
