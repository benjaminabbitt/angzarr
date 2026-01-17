Feature: Inventory Management Business Logic
  Tests inventory aggregate behavior independent of transport.
  These scenarios verify pure business logic for stock management and reservations.

  # --- InitializeStock scenarios ---

  Scenario: Initialize stock for a new product
    Given no prior events for the inventory aggregate
    When I handle an InitializeStock command with product_id "SKU-001" and quantity 100
    Then the result is a StockInitialized event
    And the inventory event has product_id "SKU-001"
    And the inventory event has quantity 100

  Scenario: Cannot initialize stock twice
    Given a StockInitialized event with product_id "SKU-002" and quantity 50
    When I handle an InitializeStock command with product_id "SKU-002" and quantity 100
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already initialized"

  Scenario: Cannot initialize with negative quantity
    Given no prior events for the inventory aggregate
    When I handle an InitializeStock command with product_id "SKU-003" and quantity -10
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "quantity"

  # --- ReceiveStock scenarios ---

  Scenario: Receive additional stock
    Given a StockInitialized event with product_id "SKU-010" and quantity 50
    When I handle a ReceiveStock command with quantity 25 and reference "PO-12345"
    Then the result is a StockReceived event
    And the inventory event has quantity 25
    And the inventory event has new_on_hand 75

  Scenario: Cannot receive stock for uninitialized product
    Given no prior events for the inventory aggregate
    When I handle a ReceiveStock command with quantity 10 and reference "PO-99999"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not initialized"

  Scenario: Cannot receive negative quantity
    Given a StockInitialized event with product_id "SKU-011" and quantity 50
    When I handle a ReceiveStock command with quantity -5 and reference "PO-BAD"
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "quantity"

  # --- ReserveStock scenarios ---

  Scenario: Reserve stock for an order
    Given a StockInitialized event with product_id "SKU-020" and quantity 100
    When I handle a ReserveStock command with quantity 10 and order_id "ORD-001"
    Then the result is a StockReserved event
    And the inventory event has quantity 10
    And the inventory event has order_id "ORD-001"
    And the inventory event has new_available 90

  Scenario: Reserve multiple times for different orders
    Given a StockInitialized event with product_id "SKU-021" and quantity 100
    And a StockReserved event with quantity 20 and order_id "ORD-A"
    When I handle a ReserveStock command with quantity 30 and order_id "ORD-B"
    Then the result is a StockReserved event
    And the inventory event has quantity 30
    And the inventory event has new_available 50

  Scenario: Cannot reserve more than available
    Given a StockInitialized event with product_id "SKU-022" and quantity 50
    And a StockReserved event with quantity 40 and order_id "ORD-X"
    When I handle a ReserveStock command with quantity 20 and order_id "ORD-Y"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "insufficient"

  Scenario: Cannot reserve for uninitialized product
    Given no prior events for the inventory aggregate
    When I handle a ReserveStock command with quantity 5 and order_id "ORD-Z"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not initialized"

  # --- ReleaseReservation scenarios ---

  Scenario: Release a reservation
    Given a StockInitialized event with product_id "SKU-030" and quantity 100
    And a StockReserved event with quantity 25 and order_id "ORD-REL"
    When I handle a ReleaseReservation command with order_id "ORD-REL"
    Then the result is a ReservationReleased event
    And the inventory event has order_id "ORD-REL"
    And the inventory event has quantity 25
    And the inventory event has new_available 100

  Scenario: Cannot release non-existent reservation
    Given a StockInitialized event with product_id "SKU-031" and quantity 100
    When I handle a ReleaseReservation command with order_id "ORD-GHOST"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not found"

  # --- CommitReservation scenarios ---

  Scenario: Commit a reservation reduces on-hand stock
    Given a StockInitialized event with product_id "SKU-040" and quantity 100
    And a StockReserved event with quantity 15 and order_id "ORD-COMMIT"
    When I handle a CommitReservation command with order_id "ORD-COMMIT"
    Then the result is a ReservationCommitted event
    And the inventory event has order_id "ORD-COMMIT"
    And the inventory event has quantity 15
    And the inventory event has new_on_hand 85

  Scenario: Cannot commit non-existent reservation
    Given a StockInitialized event with product_id "SKU-041" and quantity 100
    When I handle a CommitReservation command with order_id "ORD-MISSING"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not found"

  # --- Low stock alert scenarios ---

  Scenario: Low stock alert when available drops below threshold
    Given a StockInitialized event with product_id "SKU-050" and quantity 20 and low_stock_threshold 10
    When I handle a ReserveStock command with quantity 15 and order_id "ORD-LOW"
    Then the result is a StockReserved event
    And the inventory event has new_available 5
    And a LowStockAlert event is also emitted

  # --- State reconstruction scenarios ---

  Scenario: Rebuild state from initialization and stock receipt
    Given a StockInitialized event with product_id "SKU-060" and quantity 50
    And a StockReceived event with quantity 30
    When I rebuild the inventory state
    Then the inventory state has product_id "SKU-060"
    And the inventory state has on_hand 80
    And the inventory state has reserved 0
    And the inventory state has available 80

  Scenario: Rebuild state with reservation
    Given a StockInitialized event with product_id "SKU-061" and quantity 100
    And a StockReserved event with quantity 25 and order_id "ORD-STATE"
    When I rebuild the inventory state
    Then the inventory state has on_hand 100
    And the inventory state has reserved 25
    And the inventory state has available 75

  Scenario: Rebuild state after commit
    Given a StockInitialized event with product_id "SKU-062" and quantity 100
    And a StockReserved event with quantity 30 and order_id "ORD-COMMITTED"
    And a ReservationCommitted event with order_id "ORD-COMMITTED"
    When I rebuild the inventory state
    Then the inventory state has on_hand 70
    And the inventory state has reserved 0
    And the inventory state has available 70

  Scenario: Rebuild state after release
    Given a StockInitialized event with product_id "SKU-063" and quantity 100
    And a StockReserved event with quantity 40 and order_id "ORD-RELEASED"
    And a ReservationReleased event with order_id "ORD-RELEASED"
    When I rebuild the inventory state
    Then the inventory state has on_hand 100
    And the inventory state has reserved 0
    And the inventory state has available 100
