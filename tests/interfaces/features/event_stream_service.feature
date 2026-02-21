# docs:start:stream_service_contract
Feature: EventStreamService interface
  The EventStreamService provides real-time event streaming to subscribers. Unlike
  EventQueryService (which queries historical events), this service pushes events
  to subscribers as they occur.

  Subscribers register with a correlation_id filter. Events matching the filter
  are delivered in real-time. Events without matching subscribers are silently
  dropped (expected behavior for pub/sub).

  Without this service, clients would need to poll EventQueryService repeatedly,
  adding latency and load. Real-time streaming enables efficient saga coordination
  and live event processing across domains.
# docs:end:stream_service_contract

  Background:
    Given an EventStreamService backend

  # ==========================================================================
  # Subscribe - Registration
  # ==========================================================================

  Scenario: Subscribe with correlation ID creates working subscription
    When I subscribe with correlation ID "workflow-001"
    Then the subscription should be active
    And I should be able to receive events for "workflow-001"

  Scenario: Subscribe requires correlation ID
    When I subscribe without a correlation ID
    Then the subscribe should fail with INVALID_ARGUMENT

  Scenario: Subscribe with empty correlation ID fails
    When I subscribe with correlation ID ""
    Then the subscribe should fail with INVALID_ARGUMENT

  # ==========================================================================
  # Event Delivery
  # ==========================================================================

  Scenario: Subscriber receives matching events
    Given I am subscribed with correlation ID "order-process"
    When an event with correlation ID "order-process" is published
    Then I should receive the event in my stream

  Scenario: Subscriber ignores non-matching events
    Given I am subscribed with correlation ID "order-001"
    When an event with correlation ID "order-002" is published
    Then I should not receive any events

  Scenario: Events without subscribers are silently dropped
    When an event with correlation ID "orphan-event" is published
    Then no error should occur

  Scenario: Events without correlation ID are dropped
    Given I am subscribed with correlation ID "some-workflow"
    When an event without a correlation ID is published
    Then I should not receive any events

  # ==========================================================================
  # Multiple Subscribers
  # ==========================================================================

  Scenario: Multiple subscribers receive same event
    Given subscriber 1 is subscribed with correlation ID "shared-workflow"
    And subscriber 2 is subscribed with correlation ID "shared-workflow"
    When an event with correlation ID "shared-workflow" is published
    Then subscriber 1 should receive the event
    And subscriber 2 should receive the event

  Scenario: Different correlation IDs maintain isolation
    Given subscriber 1 is subscribed with correlation ID "workflow-A"
    And subscriber 2 is subscribed with correlation ID "workflow-B"
    When an event with correlation ID "workflow-A" is published
    Then subscriber 1 should receive the event
    And subscriber 2 should not receive any events

  # ==========================================================================
  # Disconnect Behavior
  # ==========================================================================

  Scenario: Disconnected subscriber stops receiving events
    Given I am subscribed with correlation ID "disconnect-test"
    When I disconnect my subscription
    And an event with correlation ID "disconnect-test" is published
    Then no events should be delivered to the disconnected subscriber

  Scenario: Other subscribers continue when one disconnects
    Given subscriber 1 is subscribed with correlation ID "partial-disconnect"
    And subscriber 2 is subscribed with correlation ID "partial-disconnect"
    When subscriber 1 disconnects
    And an event with correlation ID "partial-disconnect" is published
    Then subscriber 2 should receive the event

  # ==========================================================================
  # Event Content
  # ==========================================================================

  Scenario: Event book content is preserved through delivery
    Given I am subscribed with correlation ID "content-test"
    When an event with correlation ID "content-test" and 3 pages is published
    Then I should receive an EventBook with 3 pages
    And the EventBook should have correlation ID "content-test"

  Scenario: Domain and root are preserved in delivered events
    Given I am subscribed with correlation ID "preserve-test"
    When an event from domain "order" with root "order-123" and correlation ID "preserve-test" is published
    Then I should receive an EventBook with domain "order"
    And the EventBook should have a valid root UUID
