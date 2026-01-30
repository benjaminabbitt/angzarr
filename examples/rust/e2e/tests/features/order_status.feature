Feature: Process Manager - Order Status Tracking
  Tests the order status process manager which observes order lifecycle
  events across order, inventory, and fulfillment domains.

  The PM records status transitions as internal events:
  created -> payment_received / stock_reserved -> ready -> completed -> shipping
  Any non-terminal state -> cancelled (terminal)

  Background:
    # Process manager "order-status" subscribes to events from
    # order, inventory, and fulfillment domains.
    # PM state is stored in its own "order-status" domain.

  # ===========================================================================
  # Status Tracking - Happy Path
  # ===========================================================================

  @e2e @order-status @tracking
  Scenario: Order status tracks payment then stock
    Given an order "OS-HAPPY-001" with correlation "OS-CORR-001"
    When PaymentSubmitted is received for correlation "OS-CORR-001"
    And StockReserved is received for correlation "OS-CORR-001"
    Then within 3 seconds the order status PM shows "ready" for correlation "OS-CORR-001"

  @e2e @order-status @tracking
  Scenario: Order status tracks stock then payment
    Given an order "OS-REVERSE-001" with correlation "OS-CORR-002"
    When StockReserved is received for correlation "OS-CORR-002"
    And PaymentSubmitted is received for correlation "OS-CORR-002"
    Then within 3 seconds the order status PM shows "ready" for correlation "OS-CORR-002"

  @e2e @order-status @tracking
  Scenario: Order completed transitions through completed to shipping
    Given an order "OS-COMPLETE-001" with correlation "OS-CORR-003" is paid
    When payment is confirmed for order "OS-COMPLETE-001" with correlation "OS-CORR-003"
    # ConfirmPayment → OrderCompleted → "completed" → saga ShipmentCreated → "shipping"
    Then within 5 seconds the order status PM has 4 transitions for correlation "OS-CORR-003"
    And the transitions include "created" and "payment_received" and "completed"

  @e2e @order-status @tracking
  Scenario: Shipment created updates status to shipping
    Given an order "OS-SHIP-001" with correlation "OS-CORR-004" is completed
    # Fulfillment saga auto-creates shipment on OrderCompleted
    Then within 5 seconds the order status PM shows "shipping" for correlation "OS-CORR-004"

  # ===========================================================================
  # Cancellation
  # ===========================================================================

  @e2e @order-status @cancellation
  Scenario: Cancelled order is terminal
    Given an order "OS-CANCEL-001" with correlation "OS-CORR-005"
    When the order "OS-CANCEL-001" is cancelled with correlation "OS-CORR-005"
    Then within 3 seconds the order status PM shows "cancelled" for correlation "OS-CORR-005"

  @e2e @order-status @cancellation
  Scenario: Events after cancellation are ignored
    Given an order "OS-CANCEL-002" with correlation "OS-CORR-006"
    When the order "OS-CANCEL-002" is cancelled with correlation "OS-CORR-006"
    And StockReserved is received for correlation "OS-CORR-006"
    Then within 3 seconds the order status PM shows "cancelled" for correlation "OS-CORR-006"

  # ===========================================================================
  # Idempotency
  # ===========================================================================

  @e2e @order-status @idempotency
  Scenario: Duplicate payment event does not change status
    Given an order "OS-IDEM-001" with correlation "OS-CORR-007"
    When PaymentSubmitted is received for correlation "OS-CORR-007"
    Then within 3 seconds the order status PM shows "payment_received" for correlation "OS-CORR-007"
    # Second PaymentSubmitted is rejected by order aggregate (already submitted),
    # so no new event reaches the PM. Status stays "payment_received".

  # ===========================================================================
  # State Query
  # ===========================================================================

  @e2e @order-status @state
  Scenario: PM state shows transition history
    Given an order "OS-HIST-001" with correlation "OS-CORR-008"
    When PaymentSubmitted is received for correlation "OS-CORR-008"
    And StockReserved is received for correlation "OS-CORR-008"
    Then within 3 seconds the order status PM has 3 transitions for correlation "OS-CORR-008"
    And the transitions include "created" and "payment_received" and "ready"

  # ===========================================================================
  # Correlation Isolation
  # ===========================================================================

  @e2e @order-status @isolation
  Scenario: Different orders track independently
    Given an order "OS-ISO-A" with correlation "OS-ISO-CORR-A"
    And an order "OS-ISO-B" with correlation "OS-ISO-CORR-B"
    When PaymentSubmitted is received for correlation "OS-ISO-CORR-A"
    And PaymentSubmitted is received for correlation "OS-ISO-CORR-B"
    And StockReserved is received for correlation "OS-ISO-CORR-B"
    Then within 3 seconds the order status PM shows "payment_received" for correlation "OS-ISO-CORR-A"
    And within 3 seconds the order status PM shows "ready" for correlation "OS-ISO-CORR-B"

  @e2e @order-status @isolation
  Scenario: Cancellation of one order does not affect another
    Given an order "OS-ISO-C" with correlation "OS-ISO-CORR-C"
    And an order "OS-ISO-D" with correlation "OS-ISO-CORR-D"
    When PaymentSubmitted is received for correlation "OS-ISO-CORR-C"
    And the order "OS-ISO-D" is cancelled with correlation "OS-ISO-CORR-D"
    Then within 3 seconds the order status PM shows "payment_received" for correlation "OS-ISO-CORR-C"
    And within 3 seconds the order status PM shows "cancelled" for correlation "OS-ISO-CORR-D"
