Feature: Dead Letter Publisher Contracts

  All DLQ publisher implementations must fulfill these contracts for
  persisting failed messages. Each publisher stores dead letters in its
  backing store and must correctly preserve all message attributes.

  Background:
    Given a DLQ publisher backend

  # ============================================================================
  # Basic Persistence Contract
  # ============================================================================

  Scenario: Publish persists dead letter
    Given a dead letter for domain "orders" with reason "Sequence mismatch"
    When the dead letter is published
    Then publish succeeds
    And the dead letter is persisted

  Scenario: Publish persists correlation ID
    Given a dead letter with correlation ID "txn-12345"
    When the dead letter is published
    Then the persisted entry has correlation ID "txn-12345"

  Scenario: Publish persists rejection reason
    Given a dead letter with rejection reason "Command expects 0, aggregate at 5"
    When the dead letter is published
    Then the persisted entry contains rejection reason "Command expects 0"

  Scenario: Publish persists source component info
    Given a dead letter from source "saga-order-fulfillment" of type "saga"
    When the dead letter is published
    Then the persisted entry has source component "saga-order-fulfillment"
    And the persisted entry has source type "saga"

  # ============================================================================
  # Rejection Type Persistence
  # ============================================================================

  Scenario: Publish sequence mismatch dead letter
    Given a sequence mismatch dead letter with expected=5 actual=10
    When the dead letter is published
    Then the persisted entry has rejection type "sequence_mismatch"

  Scenario: Publish event processing failure dead letter
    Given an event processing failure dead letter with retry_count=3
    When the dead letter is published
    Then the persisted entry has rejection type "event_processing_failed"

  # ============================================================================
  # Multiple Dead Letters
  # ============================================================================

  Scenario: Publish multiple dead letters preserves order
    When 5 dead letters are published for domain "batch-test"
    Then 5 entries are persisted
    And entries are persisted in order

  Scenario: Publish to different domains
    Given dead letters for multiple domains
    When all dead letters are published
    Then each domain has 1 persisted entry

  # ============================================================================
  # is_configured Contract
  # ============================================================================

  Scenario: Publisher reports configured status
    Then is_configured returns true

  # ============================================================================
  # Timestamp Persistence
  # ============================================================================

  Scenario: Publish preserves timestamp
    Given a dead letter with occurred_at timestamp
    When the dead letter is published
    Then the persisted entry has a valid timestamp
