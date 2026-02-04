Feature: Domain Lifecycle Operations
  Exercises every command in every bounded context to ensure
  comprehensive coverage of all client logic.

  # ===========================================================================
  # Inventory Domain
  # ===========================================================================

  @e2e @domain @inventory
  Scenario: Initialize stock
    When I initialize stock for "INV-001" with 100 units
    Then the command succeeds
    And an event "StockInitialized" is emitted

  @e2e @domain @inventory
  Scenario: Receive additional stock
    Given inventory for "INV-RECV" has 50 units
    When I receive 25 units for "INV-RECV"
    Then the command succeeds
    And an event "StockReceived" is emitted

  @e2e @domain @inventory
  Scenario: Reserve stock for an order
    Given inventory for "INV-RESV" has 100 units
    When I reserve 10 units of "INV-RESV" for order "ORD-R001"
    Then the command succeeds
    And an event "StockReserved" is emitted

  @e2e @domain @inventory
  Scenario: Release reservation
    Given inventory for "INV-REL" has 100 units
    And 10 units of "INV-REL" are reserved for order "ORD-REL"
    When I release reservation of "INV-REL" for order "ORD-REL"
    Then the command succeeds
    And an event "ReservationReleased" is emitted

  @e2e @domain @inventory
  Scenario: Commit reservation
    Given inventory for "INV-COM" has 100 units
    And 10 units of "INV-COM" are reserved for order "ORD-COM"
    When I commit reservation of "INV-COM" for order "ORD-COM"
    Then the command succeeds
    And an event "ReservationCommitted" is emitted

  @e2e @domain @inventory
  Scenario: Cannot reserve more than available
    Given inventory for "INV-OVER" has 5 units
    When I reserve 10 units of "INV-OVER" for order "ORD-OVER"
    Then the command fails with "Insufficient"

  # ===========================================================================
  # Fulfillment Domain
  # ===========================================================================

  @e2e @domain @fulfillment
  Scenario: Create a shipment
    When I create shipment "SHIP-001" for order "ORD-SHIP-001"
    Then the command succeeds
    And an event "ShipmentCreated" is emitted

  @e2e @domain @fulfillment
  Scenario: Full shipment lifecycle - pick, pack, ship, deliver
    Given a shipment "SHIP-FULL" exists for order "ORD-SHIP-FULL"
    When I mark shipment "SHIP-FULL" as picked by "PICKER-001"
    Then the command succeeds
    And an event "ItemsPicked" is emitted

    When I mark shipment "SHIP-FULL" as packed by "PACKER-001"
    Then the command succeeds
    And an event "ItemsPacked" is emitted

    When I ship "SHIP-FULL" via "FedEx" tracking "TRACK-001"
    Then the command succeeds
    And an event "Shipped" is emitted

    When I record delivery for "SHIP-FULL" with signature "John Doe"
    Then the command succeeds
    And an event "Delivered" is emitted

  @e2e @domain @fulfillment
  Scenario: Cannot ship before packing
    Given a shipment "SHIP-NOPICK" exists for order "ORD-NOPICK"
    When I ship "SHIP-NOPICK" via "UPS" tracking "TRACK-ERR"
    Then the command fails with "not packed"

  # ===========================================================================
  # Order Domain
  # ===========================================================================

  @e2e @domain @order
  Scenario: Create an order
    When I create order "ORD-NEW" with 2 of "SKU-001" at 1000 cents
    Then the command succeeds
    And an event "OrderCreated" is emitted

  @e2e @domain @order
  Scenario: Apply loyalty discount to order
    Given an order "ORD-DISC" exists
    When I apply loyalty discount of 500 points worth 250 cents to order "ORD-DISC"
    Then the command succeeds
    And an event "LoyaltyDiscountApplied" is emitted

  @e2e @domain @order
  Scenario: Submit payment for order
    Given an order "ORD-PAY" exists
    When I submit payment of 2000 cents via "card" for order "ORD-PAY"
    Then the command succeeds
    And an event "PaymentSubmitted" is emitted

  @e2e @domain @order
  Scenario: Confirm payment completes order
    Given an order "ORD-CONF" exists and is paid
    When I confirm payment for order "ORD-CONF" with reference "PAY-REF-001"
    Then the command succeeds
    And an event "OrderCompleted" is emitted

  @e2e @domain @order
  Scenario: Cancel an order
    Given an order "ORD-CNCL" exists
    When I cancel order "ORD-CNCL" with reason "Changed my mind"
    Then the command succeeds
    And an event "OrderCancelled" is emitted

  @e2e @domain @order
  Scenario: Cannot cancel a completed order
    Given an order "ORD-DONE" exists and is completed
    When I cancel order "ORD-DONE" with reason "Too late"
    Then the command fails with "completed"

  # ===========================================================================
  # Saga Integration
  # ===========================================================================

  @e2e @domain @saga-manual
  Scenario: Order completion triggers fulfillment via saga
    Given an order "ORD-SAGA" exists and is paid
    When I confirm payment for order "ORD-SAGA" with reference "PAY-SAGA"
    Then the command succeeds
    And an event "OrderCompleted" is emitted
    # Saga automatically creates shipment using order root
    Then within 5 seconds:
      | domain      | event_type      |
      | fulfillment | ShipmentCreated |
