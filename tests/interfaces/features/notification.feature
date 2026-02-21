Feature: Notification transient signals
  Notifications are non-persisted messages for signaling between components.
  Unlike events, they are ephemeral - not stored, no sequence numbers, no replay.

  Use cases:
  - Compensation signals: Notify source aggregate that saga command was rejected
  - Real-time alerts: System warnings and operational notifications
  - Fire-and-forget updates: Status changes that don't require persistence

  Key behaviors:
  - NOT persisted to event store (ephemeral)
  - NO sequence numbers (not ordered within an aggregate)
  - NO event sourcing replay (fire-and-forget)
  - Routed via Cover (domain, root, correlation_id)

  Background:
    Given a Notification test environment

  # ==========================================================================
  # Notification Structure
  # ==========================================================================

  Scenario: Notification has required fields
    When I create a notification with cover and payload
    Then the notification should have a cover
    And the notification should have a payload
    And the notification should have a sent_at timestamp

  Scenario: Notification cover provides routing info
    Given a notification for domain "order" with root "order-001"
    Then the notification cover should have domain "order"
    And the notification cover should have root "order-001"

  Scenario: Notification can include correlation ID
    Given a notification with correlation ID "workflow-123"
    Then the notification cover should have correlation ID "workflow-123"

  Scenario: Notification metadata is optional
    When I create a notification without metadata
    Then the notification metadata should be empty

  Scenario: Notification can include metadata
    Given a notification with metadata:
      | key     | value   |
      | retry   | 1       |
      | source  | saga-a  |
    Then the notification should have metadata key "retry" with value "1"
    And the notification should have metadata key "source" with value "saga-a"

  # ==========================================================================
  # RejectionNotification
  # ==========================================================================

  Scenario: Rejection notification includes rejected command
    Given a saga command was rejected with reason "insufficient_funds"
    When I build a rejection notification
    Then the rejection should include the original command
    And the rejection reason should be "insufficient_funds"

  Scenario: Rejection notification identifies issuer
    Given a saga "order-fulfillment" issued a command
    And the command was rejected
    When I build a rejection notification
    Then the rejection issuer name should be "order-fulfillment"
    And the rejection issuer type should be "saga"

  Scenario: Rejection notification links to source aggregate
    Given a saga triggered by aggregate "order" with root "order-123" at sequence 5
    And the saga command was rejected
    When I build a rejection notification
    Then the rejection source aggregate should be "order" with root "order-123"
    And the rejection source event sequence should be 5

  Scenario: Rejection notification has correct type URL
    Given a saga command was rejected
    When I wrap the rejection in a notification
    Then the notification payload type URL should be "type.googleapis.com/angzarr.RejectionNotification"

  # ==========================================================================
  # Notification Routing
  # ==========================================================================

  Scenario: Notification routes to triggering aggregate
    Given a saga triggered by aggregate "order" with root "order-123"
    And the saga command was rejected
    When I build a notification command book
    Then the command book cover should target "order" with root "order-123"

  Scenario: Notification preserves correlation ID for tracing
    Given a saga command with correlation ID "trace-456"
    And the command was rejected
    When I build a notification command book
    Then the command book cover should have correlation ID "trace-456"

  Scenario: Notification uses MERGE_COMMUTATIVE strategy
    Given a saga command was rejected
    When I build a notification command book
    Then the command page should use MERGE_COMMUTATIVE

  # ==========================================================================
  # Notification vs Event Semantics
  # ==========================================================================

  Scenario: Notifications are not persisted
    Given a notification is created
    Then the notification should not be stored in the event store

  Scenario: Notifications have no sequence
    Given a notification is created
    Then the notification should not have a sequence number

  Scenario: Notifications are fire-and-forget
    Given a notification is sent
    Then the notification cannot be replayed
