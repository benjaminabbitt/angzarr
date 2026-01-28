Feature: Projector Read Models
  Tests that projectors correctly build and maintain read models
  from the event stream. Validates SQLite projector output.

  Background:
    # Projectors write to in-memory SQLite in standalone mode

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
    Given an order "ORD-STATUS" is created for projector test
    Then within 2 seconds the web projector shows status "pending" for "ORD-STATUS"

    When order "ORD-STATUS" payment is submitted
    Then within 2 seconds the web projector shows status "paid" for "ORD-STATUS"

    When order "ORD-STATUS" payment is confirmed
    Then within 2 seconds the web projector shows status "completed" for "ORD-STATUS"

  @e2e @projector @web @gateway
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
    Given a completed order "ORD-REV" totaling 5000 cents
    Then within 2 seconds the accounting ledger for "ORD-REV" has:
      | entry_type | amount_cents |
      | revenue    | 5000         |

  @e2e @projector @accounting
  Scenario: Accounting projector tracks discounts
    Given a completed order with:
      | field          | value     |
      | order_id       | ORD-DISC-P|
      | subtotal_cents | 10000     |
      | discount_cents | 1500      |
      | total_cents    | 8500      |
    Then within 2 seconds the accounting ledger for "ORD-DISC-P" has:
      | entry_type | amount_cents |
      | revenue    | 8500         |
      | discount   | 1500         |

  @e2e @projector @accounting
  Scenario: Accounting projector handles refunds
    Given an order "ORD-REFUND" is created totaling 3000 cents
    When order "ORD-REFUND" is cancelled
    Then within 2 seconds the accounting ledger for "ORD-REFUND" has:
      | entry_type | amount_cents |
      | revenue    | 3000         |
      | refund     | -3000        |

  # ===========================================================================
  # Accounting Projector - Loyalty Balance
  # ===========================================================================

  @e2e @projector @loyalty
  Scenario: Loyalty balance updated on points earned
    Given customer "CUST-BAL" exists with 0 loyalty points
    When I add 250 loyalty points to customer "CUST-BAL" for "purchase reward"
    Then within 2 seconds the loyalty balance for "CUST-BAL" shows:
      | field           | value |
      | current_points  | 250   |
      | lifetime_points | 250   |

  @e2e @projector @loyalty
  Scenario: Loyalty balance updated on points redeemed
    Given customer "CUST-USE" exists with 500 loyalty points
    When I redeem 200 loyalty points from customer "CUST-USE" for "discount"
    Then within 2 seconds the loyalty balance for "CUST-USE" shows:
      | field          | value |
      | current_points | 300   |

  @e2e @projector @loyalty @gateway
  Scenario: Loyalty balance restored on refund
    Given customer "CUST-RESTORE" used 150 points on cancelled order
    When the order is cancelled and points refunded
    Then within 2 seconds the loyalty balance is restored
    And current_points increased by 150

  # ===========================================================================
  # Projector Consistency
  # ===========================================================================

  @e2e @projector @consistency @gateway
  Scenario: Projectors handle out-of-order events
    Given events arrive out of order:
      | sequence | event_type    |
      | 2        | ItemAdded     |
      | 0        | CartCreated   |
      | 1        | ItemAdded     |
    Then the projector reorders and processes correctly
    And the final state reflects all events

  @e2e @projector @consistency @gateway
  Scenario: Projectors handle duplicate events idempotently
    Given an event "OrderCompleted" for order "ORD-DUP-EVT"
    When the same event is delivered twice
    Then the projector state is unchanged after second delivery
    And no duplicate ledger entries exist

  @e2e @projector @consistency @gateway
  Scenario: Projectors maintain position tracking
    Given the web projector has processed up to position 100
    When new events arrive at positions 101-105
    Then only positions 101-105 are processed
    And the position tracker shows 105

  # ===========================================================================
  # Projector Recovery
  # ===========================================================================

  @e2e @projector @recovery @gateway
  Scenario: Projector rebuilds from event stream
    Given the web projector database is cleared
    When I trigger a full rebuild
    Then the projector replays all events from position 0
    And the final state matches expected

  @e2e @projector @recovery @gateway
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
    Then within 3 seconds the web projector total for "ORD-CROSS" is 7500
    And within 3 seconds the accounting revenue for "ORD-CROSS" is 7500

  @e2e @projector @cross @gateway
  Scenario: All projectors process same correlation ID
    Given an order flow with correlation "CROSS-CORR"
    When the order is completed
    Then all projector updates reference correlation "CROSS-CORR"
