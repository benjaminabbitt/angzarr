Feature: Event Idempotency

  Commands and facts can include an external_id for exactly-once delivery semantics.
  When the same external_id is submitted twice, the second request returns the original
  event sequences without persisting duplicates.

  This prevents duplicate side effects when:
  - API clients retry due to network failures or timeouts
  - External systems (webhooks, message queues) redeliver messages
  - Load balancers or proxies duplicate requests

  Without idempotency, a payment command retried 3 times would create 3 payment events.

  Idempotency is scoped to (domain, edition, root, external_id) - the same external_id
  can be used for different aggregates without collision.

  Background:
    Given an EventStore backend

  # ==========================================================================
  # Basic Idempotency Contract
  # ==========================================================================

  Scenario: First request with external_id persists events
    Given an aggregate "payment" with no events
    When I add 1 event with external_id "request-001"
    Then the aggregate should have 1 event
    And the add outcome should be "added"
    And the outcome should report first_sequence 0
    And the outcome should report last_sequence 0

  Scenario: Duplicate external_id returns original sequences without adding events
    Given an aggregate "payment" with no events
    When I add 2 events with external_id "request-002"
    And I add 2 events with external_id "request-002"
    Then the aggregate should have 2 events
    And the add outcome should be "duplicate"
    And the outcome should report first_sequence 0
    And the outcome should report last_sequence 1

  Scenario: Different external_ids on same aggregate are independent
    Given an aggregate "payment" with no events
    When I add 1 event with external_id "request-A"
    And I add 1 event with external_id "request-B"
    Then the aggregate should have 2 events
    And events should have consecutive sequences starting from 0

  # ==========================================================================
  # Scope Isolation
  # ==========================================================================

  Scenario: Same external_id on different aggregates does not collide
    Given an aggregate "payment" with root "payment-001" and no events
    And an aggregate "payment" with root "payment-002" and no events
    When I add 1 event to "payment-001" with external_id "shared-id"
    And I add 1 event to "payment-002" with external_id "shared-id"
    Then "payment-001" in domain "payment" should have 1 event
    And "payment-002" in domain "payment" should have 1 event

  Scenario: Same external_id in different domains does not collide
    Given an aggregate "payment" with root "root-001" and no events
    And an aggregate "refund" with root "root-001" and no events
    When I add 1 event to "root-001" in domain "payment" with external_id "shared-id"
    And I add 1 event to "root-001" in domain "refund" with external_id "shared-id"
    Then "root-001" in domain "payment" should have 1 event
    And "root-001" in domain "refund" should have 1 event

  Scenario: Same external_id in different editions does not collide
    Given an aggregate "payment" with root "root-001" in edition "main"
    When I add 1 event to "root-001" in edition "main" with external_id "ext-001"
    And I add 1 event to "root-001" in edition "branch" with external_id "ext-001"
    Then "root-001" in edition "main" should have 1 event
    And "root-001" in edition "branch" should have 1 event

  # ==========================================================================
  # Requests Without Idempotency Key
  # ==========================================================================

  Scenario: Events without external_id always persist
    Given an aggregate "order" with no events
    When I add 1 event without external_id
    And I add 1 event without external_id
    Then the aggregate should have 2 events
    And events should have consecutive sequences starting from 0

  Scenario: Mixed external_id and no external_id events coexist
    Given an aggregate "order" with no events
    When I add 1 event without external_id
    And I add 1 event with external_id "idempotent-001"
    And I add 1 event without external_id
    Then the aggregate should have 3 events
    And events should have consecutive sequences starting from 0

  # ==========================================================================
  # Sequence Correctness
  # ==========================================================================

  Scenario: Duplicate detection works after other events have been added
    Given an aggregate "ledger" with no events
    When I add 2 events with external_id "batch-001"
    And I add 3 events without external_id
    And I add 2 events with external_id "batch-001"
    Then the aggregate should have 5 events
    And the add outcome should be "duplicate"
    And the outcome should report first_sequence 0
    And the outcome should report last_sequence 1

  Scenario: Multiple batches with different external_ids maintain correct sequences
    Given an aggregate "ledger" with no events
    When I add 2 events with external_id "batch-A"
    And I add 3 events with external_id "batch-B"
    Then the aggregate should have 5 events
    And the first event should have sequence 0
    And the last event should have sequence 4

  # ==========================================================================
  # Empty External ID
  # ==========================================================================

  Scenario: Empty string external_id is treated as no external_id
    Given an aggregate "order" with no events
    When I add 1 event with external_id ""
    And I add 1 event with external_id ""
    Then the aggregate should have 2 events

  # ==========================================================================
  # Concurrency Safety
  # ==========================================================================

  Scenario: Concurrent duplicate requests result in exactly one set of events
    Given an aggregate "payment" with no events
    When I add 3 events with external_id "concurrent-001" concurrently 5 times
    Then the aggregate should have 3 events
    And events should have consecutive sequences starting from 0
