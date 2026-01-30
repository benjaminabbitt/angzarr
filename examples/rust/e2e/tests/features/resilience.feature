Feature: System Resilience
  Tests edge cases and failure modes to ensure the system is bomb-proof.
  These tests validate sequence validation, idempotency, and concurrent handling.

  Background:
    # Tests run against standalone mode with SQLite storage

  # ===========================================================================
  # Idempotency Tests
  # ===========================================================================

  @e2e @resilience @idempotency
  Scenario: Duplicate command is rejected safely
    Given a cart "CART-DUP" with sequence 0
    When I add item "SKU-001" with sequence 1
    Then the command succeeds
    When I replay the exact same command with sequence 1
    Then the command fails with "Aborted"
    And the error contains missing events
    And the cart still has exactly 1 item

  @e2e @resilience @idempotency
  Scenario: Duplicate command returns correct missing events
    Given a cart "CART-DUP-2" at sequence 3
    When I add item "SKU-002" with sequence 0
    Then the command fails with "Aborted"
    And the error contains events 0-2

  # ===========================================================================
  # Sequence Validation Tests
  # ===========================================================================

  @e2e @resilience @sequence
  Scenario: Out-of-order command is rejected
    Given a cart "CART-SEQ" at sequence 2
    When I send a command expecting sequence 5
    Then the command fails with "Aborted"
    And the error contains events 2-4
    And no new events are stored

  @e2e @resilience @sequence
  Scenario: High sequence on new aggregate is rejected
    Given no aggregate exists for root "NEW-AGG-001"
    When I send a command expecting sequence 100
    Then the command fails with "Aborted"
    And the error indicates expected=100 actual=0

  @e2e @resilience @sequence
  Scenario: Command with correct sequence succeeds
    Given a cart "CART-CORRECT" at sequence 2
    When I add item "SKU-CORRECT" with sequence 2
    Then the command succeeds

  @e2e @resilience @sequence
  Scenario: Sequence zero on new aggregate succeeds
    Given no aggregate exists for root "NEW-AGG-002"
    When I send a command expecting sequence 0
    Then the command succeeds

  # ===========================================================================
  # Concurrent Write Tests
  # ===========================================================================

  @e2e @resilience @concurrent @gateway
  Scenario: Concurrent writes are serialized correctly
    Given a cart "CART-CONC" with sequence 0
    When I send 10 AddItem commands concurrently
    Then some commands succeed and some fail with sequence mismatch
    And the cart has consistent state (no duplicates, no gaps)
    And event sequences are contiguous (0, 1, 2, ...)

  @e2e @resilience @concurrent @gateway
  Scenario: High concurrency stress test
    Given a cart "CART-STRESS" with sequence 0
    When I send 50 AddItem commands concurrently
    Then some commands succeed and some fail with sequence mismatch
    And event sequences are contiguous (0, 1, 2, ...)

  # ===========================================================================
  # Saga Retry Tests
  # ===========================================================================

  @e2e @resilience @saga-retry @infra
  Scenario: Saga retries on sequence conflict
    # This test requires setting up a scenario where the saga's command
    # conflicts with a concurrent write to the target aggregate
    Given an order ready for fulfillment
    And a concurrent write to fulfillment domain
    When the order is completed
    Then the fulfillment saga retries with backoff
    And eventually a shipment is created
    And no duplicate shipments exist

  # ===========================================================================
  # Chaos Tests (require @chaos tag to run)
  # ===========================================================================

  @chaos @process
  Scenario: Saga completes after coordinator restart
    Given a pending fulfillment saga for order "ORD-CHAOS-001"
    When I kill the saga coordinator process
    And I restart the saga coordinator
    Then within 30 seconds the shipment is created
    And no duplicate shipments exist

  @chaos @network
  Scenario: System handles network delays
    Given network latency of 500ms to fulfillment domain
    When an order is completed
    Then the fulfillment saga eventually succeeds
    And correlation ID is preserved

  @e2e @resilience @corruption
  Scenario: Malformed protobuf is rejected
    Given a cart "CART-CORRUPT" with sequence 0
    When I send a command with corrupted protobuf data
    Then the command fails with "failed to decode"
    And no new events are stored
