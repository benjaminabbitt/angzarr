Feature: Projector Read Models
  Tests that projectors correctly build and maintain read models
  from the event stream. Validates SQLite projector output.

  Background:
    # Projectors write to SQLite databases in standalone mode:
    # - /tmp/angzarr/projectors/web.db
    # - /tmp/angzarr/projectors/accounting.db

  # ===========================================================================
  # Web Projector - Order Summary
  # ===========================================================================

  @e2e @projector @web
  Scenario: Web projector builds order summary
    Given a completed order with:
      | field           | value       |
      | order_id        | ORD-PROJ-001|
      | customer_id     | CUST-001    |
      | subtotal_cents  | 5000        |
      | discount_cents  | 500         |
      | total_cents     | 4500        |
    Then within 2 seconds the web projector shows:
      | field           | value       |
      | order_id        | ORD-PROJ-001|
      | customer_id     | CUST-001    |
      | status          | completed   |
      | subtotal_cents  | 5000        |
      | discount_cents  | 500         |
      | total_cents     | 4500        |

  @e2e @projector @web
  Scenario: Web projector tracks order status changes
    Given an order "ORD-STATUS" is created
    Then the web projector shows status "pending" for "ORD-STATUS"

    When order "ORD-STATUS" is paid
    Then within 1 second the web projector shows status "paid" for "ORD-STATUS"

    When order "ORD-STATUS" is completed
    Then within 1 second the web projector shows status "completed" for "ORD-STATUS"

  @e2e @projector @web
  Scenario: Web projector tracks loyalty points
    Given a completed order with:
      | field                | value  |
      | order_id             | ORD-LYL|
      | loyalty_points_used  | 200    |
      | loyalty_points_earned| 50     |
    Then within 2 seconds the web projector shows:
      | field                | value |
      | loyalty_points_used  | 200   |
      | loyalty_points_earned| 50    |

  # ===========================================================================
  # Accounting Projector - Ledger
  # ===========================================================================

  @e2e @projector @accounting
  Scenario: Accounting projector tracks revenue
    Given a completed order totaling 5000 cents
    Then within 2 seconds the accounting ledger has:
      | entry_type | amount_cents |
      | revenue    | 5000         |

  @e2e @projector @accounting
  Scenario: Accounting projector tracks discounts
    Given a completed order with:
      | subtotal_cents | discount_cents | total_cents |
      | 10000          | 1500           | 8500        |
    Then within 2 seconds the accounting ledger has entries:
      | entry_type | amount_cents |
      | revenue    | 8500         |
      | discount   | 1500         |

  @e2e @projector @accounting
  Scenario: Accounting projector handles refunds
    Given a completed order "ORD-REFUND" totaling 3000 cents
    When order "ORD-REFUND" is refunded
    Then within 2 seconds the accounting ledger has:
      | entry_type | amount_cents |
      | revenue    | 3000         |
      | refund     | -3000        |
    And the net revenue for "ORD-REFUND" is 0

  # ===========================================================================
  # Accounting Projector - Loyalty Balance
  # ===========================================================================

  @e2e @projector @loyalty
  Scenario: Loyalty balance updated on points earned
    Given customer "CUST-BAL" has 1000 lifetime points
    When customer "CUST-BAL" earns 250 points
    Then within 2 seconds the loyalty balance shows:
      | customer_id | current_points | lifetime_points |
      | CUST-BAL    | 1250           | 1250            |

  @e2e @projector @loyalty
  Scenario: Loyalty balance updated on points used
    Given customer "CUST-USE" has 500 current points
    When customer "CUST-USE" uses 200 points on order
    Then within 2 seconds the loyalty balance shows:
      | customer_id | current_points |
      | CUST-USE    | 300            |

  @e2e @projector @loyalty
  Scenario: Loyalty balance restored on refund
    Given customer "CUST-RESTORE" used 150 points on cancelled order
    When the order is cancelled and points refunded
    Then within 2 seconds the loyalty balance is restored
    And current_points increased by 150

  # ===========================================================================
  # Projector Consistency
  # ===========================================================================

  @e2e @projector @consistency
  Scenario: Projectors handle out-of-order events
    Given events arrive out of order:
      | sequence | event_type    |
      | 2        | ItemAdded     |
      | 0        | CartCreated   |
      | 1        | ItemAdded     |
    Then the projector reorders and processes correctly
    And the final state reflects all events

  @e2e @projector @consistency
  Scenario: Projectors handle duplicate events idempotently
    Given an event "OrderCompleted" for order "ORD-DUP-EVT"
    When the same event is delivered twice
    Then the projector state is unchanged after second delivery
    And no duplicate ledger entries exist

  @e2e @projector @consistency
  Scenario: Projectors maintain position tracking
    Given the web projector has processed up to position 100
    When new events arrive at positions 101-105
    Then only positions 101-105 are processed
    And the position tracker shows 105

  # ===========================================================================
  # Projector Recovery
  # ===========================================================================

  @e2e @projector @recovery
  Scenario: Projector rebuilds from event stream
    Given the web projector database is cleared
    When I trigger a full rebuild
    Then the projector replays all events from position 0
    And the final state matches expected

  @e2e @projector @recovery
  Scenario: Projector catches up after gap
    Given the web projector is at position 50
    And events exist up to position 100
    When the projector resumes
    Then events 51-100 are processed
    And no events are skipped

  # ===========================================================================
  # Cross-Projector Consistency
  # ===========================================================================

  @e2e @projector @cross
  Scenario: Web and accounting projectors agree on order totals
    Given a completed order "ORD-CROSS" with total 7500 cents
    Then within 3 seconds:
      | projector  | field       | value |
      | web        | total_cents | 7500  |
      | accounting | revenue     | 7500  |

  @e2e @projector @cross
  Scenario: All projectors process same correlation ID
    Given an order flow with correlation "CROSS-CORR"
    When the order is completed
    Then all projector updates reference correlation "CROSS-CORR"
