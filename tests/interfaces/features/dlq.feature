Feature: Dead Letter Queue
  The Dead Letter Queue (DLQ) captures failed messages for manual review and replay.
  DLQ is separate from the EventBus - handlers explicitly publish to DLQ on failure.

  Background:
    Given a DLQ publisher

  # ============================================================================
  # Basic Publishing
  # ============================================================================

  Scenario: Publish a dead letter for sequence mismatch
    Given a command with sequence 0 for domain "orders"
    When the aggregate rejects it with actual sequence 5
    And the dead letter is published to DLQ
    Then the DLQ receives a message with reason "Sequence mismatch"
    And the payload contains the original command
    And the rejection details show expected 0 and actual 5

  Scenario: Publish a dead letter for handler failure
    Given an event book for domain "orders"
    When the saga handler fails with error "Connection refused"
    And the dead letter is published to DLQ
    Then the DLQ receives a message with reason containing "Connection refused"
    And the payload contains the original events
    And the source component type is "saga"

  Scenario: Publish a dead letter for payload retrieval failure
    Given an event book with external payload reference
    When the payload retrieval fails from "gcs" with error "Object not found"
    And the dead letter is published to DLQ
    Then the DLQ receives a message with reason containing "Object not found"
    And the rejection details show storage type "gcs"

  # ============================================================================
  # Topic Routing
  # ============================================================================

  Scenario: Dead letters are routed to domain-specific topics
    Given a command for domain "orders"
    When the dead letter is published
    Then it is published to topic "angzarr.dlq.orders"

  Scenario: Dead letters preserve correlation ID
    Given a command with correlation ID "txn-12345"
    When the dead letter is published
    Then the DLQ message has correlation ID "txn-12345"

  # ============================================================================
  # Metadata
  # ============================================================================

  Scenario: Dead letters include timestamp
    When a dead letter is published
    Then the occurred_at timestamp is within the last minute

  Scenario: Dead letters support custom metadata
    Given a command that fails
    When metadata "retry_attempt" = "3" is added
    And the dead letter is published
    Then the DLQ message metadata contains "retry_attempt" = "3"

  Scenario: Dead letters include source component info
    Given a dead letter from component "saga-order-fulfillment" of type "saga"
    When it is published
    Then the DLQ message shows source "saga-order-fulfillment"
    And the DLQ message shows source type "saga"

  # ============================================================================
  # Channel Backend (Standalone Mode)
  # ============================================================================

  Scenario: Channel publisher delivers to receiver
    Given a channel DLQ publisher and receiver
    When a dead letter is published
    Then the receiver receives the dead letter
    And the payload is intact

  Scenario: Channel publisher handles multiple dead letters
    Given a channel DLQ publisher and receiver
    When 5 dead letters are published
    Then the receiver receives all 5 dead letters in order

  # ============================================================================
  # Noop Backend
  # ============================================================================

  Scenario: Noop publisher logs but doesn't fail
    Given a noop DLQ publisher
    When a dead letter is published
    Then publish succeeds
    And is_configured returns false

  # ============================================================================
  # Configuration
  # ============================================================================

  Scenario: Config for channel backend
    When DlqConfig::channel() is created
    Then the backend is Channel
    And is_configured returns true

  Scenario: Config for AMQP backend
    When DlqConfig::amqp("amqp://localhost:5672") is created
    Then the backend is Amqp
    And amqp_url is "amqp://localhost:5672"

  Scenario: Config for Kafka backend
    When DlqConfig::kafka("localhost:9092") is created
    Then the backend is Kafka
    And kafka_brokers is "localhost:9092"

  Scenario: Config for Pub/Sub backend
    When DlqConfig::pubsub() is created
    Then the backend is PubSub

  Scenario: Config for SNS/SQS backend
    When DlqConfig::sns_sqs().with_aws_region("us-east-1") is created
    Then the backend is SnsSqs
    And aws_region is "us-east-1"
