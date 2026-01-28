Feature: Temporal Query and Dry-Run
  Tests querying aggregate state at a point in time and
  speculatively executing commands without side effects.

  Background:
    # Temporal queries replay events from sequence 0 (no snapshots).
    # Dry-run executes a command against temporal state without persisting.

  # ===========================================================================
  # Temporal Query by Sequence
  # ===========================================================================

  @e2e @temporal @sequence
  Scenario: Query aggregate state at a sequence number
    Given a cart "TEMP-CART-1" with events:
      | sequence | event_type    |
      | 0        | CartCreated   |
      | 1        | ItemAdded     |
      | 2        | ItemAdded     |
      | 3        | QuantityUpdated|
      | 4        | CouponApplied |
    When I query cart "TEMP-CART-1" at sequence 2
    Then 3 events are returned (sequences 0, 1, 2)
    And no events after sequence 2 are included

  @e2e @temporal @sequence
  Scenario: Query at sequence 0 returns only creation event
    Given a cart "TEMP-CART-2" with 5 events
    When I query cart "TEMP-CART-2" at sequence 0
    Then 1 event is returned
    And the event is "CartCreated"

  @e2e @temporal @sequence
  Scenario: Query at sequence beyond current returns all events
    Given a cart "TEMP-CART-3" with 3 events
    When I query cart "TEMP-CART-3" at sequence 100
    Then 3 events are returned

  # ===========================================================================
  # Temporal Query by Timestamp
  # ===========================================================================

  @e2e @temporal @timestamp
  Scenario: Query aggregate state at a timestamp
    Given a cart "TEMP-CART-TS" with events spread across time
    When I query cart "TEMP-CART-TS" as-of a timestamp before the third event
    Then only events before that timestamp are returned

  # ===========================================================================
  # Dry-Run - What-If Execution
  # ===========================================================================

  @e2e @dryrun
  Scenario: Dry-run succeeds against temporal state
    Given a cart "DRY-CART-1" with items:
      | sequence | event_type | details         |
      | 0        | CartCreated| created         |
      | 1        | ItemAdded  | sku=WIDGET-A    |
      | 2        | ItemAdded  | sku=WIDGET-B    |
    When I dry-run "RemoveItem WIDGET-B" on cart "DRY-CART-1" at sequence 2
    Then the dry-run returns an "ItemRemoved" event
    And the actual cart state is unchanged (still has WIDGET-B)

  @e2e @dryrun
  Scenario: Dry-run fails when item not in temporal state
    Given a cart "DRY-CART-2" with items:
      | sequence | event_type | details         |
      | 0        | CartCreated| created         |
      | 1        | ItemAdded  | sku=WIDGET-A    |
      | 2        | ItemAdded  | sku=WIDGET-B    |
    When I dry-run "RemoveItem WIDGET-B" at sequence 1 (before B was added)
    Then the dry-run returns an error
    And the actual cart state is unchanged

  @e2e @dryrun
  Scenario: Dry-run does not persist events
    Given a cart "DRY-CART-3" with 3 events
    When I dry-run "AddItem SKU-DRY" on cart "DRY-CART-3" at sequence 2
    Then the dry-run returns an "ItemAdded" event
    And querying cart "DRY-CART-3" still returns exactly 3 events

  @e2e @dryrun
  Scenario: Dry-run does not trigger sagas
    Given a cart "DRY-CART-4" ready for checkout
    When I dry-run "Checkout" on cart "DRY-CART-4" at latest sequence
    Then the dry-run returns a "CheckedOut" event
    And no saga commands are generated
    And no events appear in any other domain

  # ===========================================================================
  # Dry-Run for Historical What-If
  # ===========================================================================

  @e2e @dryrun @historical
  Scenario: Dry-run against earlier state for experimentation
    Given a cart "DRY-HIST" that was checked out at sequence 5
    When I dry-run "AddItem SKU-LATE" at sequence 3 (before checkout)
    Then the dry-run returns an "ItemAdded" event
    And the cart "DRY-HIST" remains checked out (no mutation)

  @e2e @dryrun @historical
  Scenario: Dry-run at sequence 0 simulates fresh aggregate
    Given no cart exists for "DRY-FRESH"
    When I dry-run "CreateCart" at sequence 0
    Then the dry-run returns a "CartCreated" event
    And the actual cart state is unchanged
