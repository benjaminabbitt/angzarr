Feature: Temporal Order Cancellation
  Speculative execution against historical aggregate state.

  Dry-run (speculative execution) reconstructs aggregate state at a past
  point in time and runs the command handler against that state. It is
  purely speculative: no events are emitted, persisted, or published.
  No sagas, projectors, or process managers are triggered. The actual
  aggregate remains unchanged.

  Use speculative execution to:
    - Validate whether a command would succeed at a historical state
    - Explore "what-if" scenarios (e.g. "could we have cancelled before completion?")
    - Audit business rule enforcement across the aggregate lifecycle

  Background:
    # Order events: 0=OrderCreated, 1=PaymentSubmitted, 2=OrderCompleted (if completed)
    # Speculative execution does NOT emit events or cause side effects.

  # ===========================================================================
  # Temporal State Reconstruction After Cancellation
  # ===========================================================================

  @e2e @temporal @order @cancellation
  Scenario: Temporal query shows pre-cancellation state
    Given an order "ORD-TC-1" exists and is paid
    When I cancel order "ORD-TC-1" with reason "Changed my mind"
    Then the command succeeds
    When I query order "ORD-TC-1" events at sequence 0
    Then the temporal result has 1 event
    And the temporal event at index 0 is "OrderCreated"
    When I query order "ORD-TC-1" events at sequence 1
    Then the temporal result has 2 events
    And the temporal event at index 1 is "PaymentSubmitted"
    When I query order "ORD-TC-1" all events
    Then the order has 3 total events
    And the temporal event at index 2 is "OrderCancelled"

  # ===========================================================================
  # Speculative Cancellation Against Temporal State
  # ===========================================================================

  @e2e @temporal @order @dryrun
  Scenario: Speculative cancellation succeeds at payment-submitted state
    # Order went through: Created -> PaymentSubmitted -> Completed (3 events).
    # Speculatively cancel at sequence 1 (payment_submitted, not yet completed).
    # The cancellation handler accepts this state, returning an OrderCancelled
    # event speculatively. No events are persisted or published.
    Given an order "ORD-TC-2" exists and is completed
    When I dry-run cancel order "ORD-TC-2" at sequence 1 with reason "What if cancelled before completion?"
    Then the dry-run returns an "OrderCancelled" event
    And querying order "ORD-TC-2" still returns exactly 3 events

  @e2e @temporal @order @dryrun
  Scenario: Speculative cancellation fails at completed state
    # At sequence 2 the order is completed. Business rules reject cancellation
    # of completed orders. The speculative execution returns an error without
    # any side effects.
    Given an order "ORD-TC-3" exists and is completed
    When I dry-run cancel order "ORD-TC-3" at sequence 2 with reason "Too late"
    Then the dry-run returns an error
    And querying order "ORD-TC-3" still returns exactly 3 events

  @e2e @temporal @order @dryrun
  Scenario: Speculative cancellation at creation state succeeds
    # At sequence 0 the order just exists (pending). Cancellation is allowed.
    # Speculative execution confirms this without mutating the aggregate.
    Given an order "ORD-TC-4" exists and is completed
    When I dry-run cancel order "ORD-TC-4" at sequence 0 with reason "Cancel immediately"
    Then the dry-run returns an "OrderCancelled" event
    And querying order "ORD-TC-4" still returns exactly 3 events
