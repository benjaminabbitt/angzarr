Feature: Fulfillment client logic
  Tests fulfillment aggregate behavior independent of transport.
  Manages shipment lifecycle: pending → picking → packing → shipped → delivered.

  # --- CreateShipment scenarios ---

  Scenario: Create a new shipment for an order
    Given no prior events for the fulfillment aggregate
    When I handle a CreateShipment command with order_id "ORD-001"
    Then the result is a ShipmentCreated event
    And the fulfillment event has order_id "ORD-001"
    And the fulfillment event has status "pending"

  Scenario: Cannot create shipment twice for same order
    Given a ShipmentCreated event with order_id "ORD-002"
    When I handle a CreateShipment command with order_id "ORD-002"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already exists"

  # --- MarkPicked scenarios ---

  Scenario: Mark shipment as picked
    Given a ShipmentCreated event with order_id "ORD-010"
    When I handle a MarkPicked command with picker_id "PICKER-001"
    Then the result is an ItemsPicked event
    And the fulfillment event has picker_id "PICKER-001"

  Scenario: Cannot pick shipment not in pending status
    Given a ShipmentCreated event with order_id "ORD-011"
    And an ItemsPicked event
    When I handle a MarkPicked command with picker_id "PICKER-002"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not pending"

  Scenario: Cannot pick non-existent shipment
    Given no prior events for the fulfillment aggregate
    When I handle a MarkPicked command with picker_id "PICKER-003"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  # --- MarkPacked scenarios ---

  Scenario: Mark shipment as packed
    Given a ShipmentCreated event with order_id "ORD-020"
    And an ItemsPicked event
    When I handle a MarkPacked command with packer_id "PACKER-001"
    Then the result is an ItemsPacked event
    And the fulfillment event has packer_id "PACKER-001"

  Scenario: Cannot pack shipment not in picking status
    Given a ShipmentCreated event with order_id "ORD-021"
    When I handle a MarkPacked command with packer_id "PACKER-002"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not picked"

  Scenario: Cannot pack already packed shipment
    Given a ShipmentCreated event with order_id "ORD-022"
    And an ItemsPicked event
    And an ItemsPacked event
    When I handle a MarkPacked command with packer_id "PACKER-003"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not picked"

  # --- Ship scenarios ---

  Scenario: Ship a packed shipment
    Given a ShipmentCreated event with order_id "ORD-030"
    And an ItemsPicked event
    And an ItemsPacked event
    When I handle a Ship command with carrier "FedEx" and tracking_number "TRACK-001"
    Then the result is a Shipped event
    And the fulfillment event has carrier "FedEx"
    And the fulfillment event has tracking_number "TRACK-001"

  Scenario: Cannot ship unpacked shipment
    Given a ShipmentCreated event with order_id "ORD-031"
    And an ItemsPicked event
    When I handle a Ship command with carrier "UPS" and tracking_number "TRACK-002"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not packed"

  Scenario: Cannot ship already shipped
    Given a ShipmentCreated event with order_id "ORD-032"
    And an ItemsPicked event
    And an ItemsPacked event
    And a Shipped event
    When I handle a Ship command with carrier "DHL" and tracking_number "TRACK-003"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not packed"

  # --- RecordDelivery scenarios ---

  Scenario: Record delivery completes fulfillment
    Given a ShipmentCreated event with order_id "ORD-040"
    And an ItemsPicked event
    And an ItemsPacked event
    And a Shipped event
    When I handle a RecordDelivery command with signature "John Doe"
    Then the result is a Delivered event
    And the fulfillment event has signature "John Doe"

  Scenario: Cannot deliver unshipped
    Given a ShipmentCreated event with order_id "ORD-041"
    And an ItemsPicked event
    And an ItemsPacked event
    When I handle a RecordDelivery command with signature "Jane Doe"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not shipped"

  Scenario: Cannot deliver already delivered
    Given a ShipmentCreated event with order_id "ORD-042"
    And an ItemsPicked event
    And an ItemsPacked event
    And a Shipped event
    And a Delivered event
    When I handle a RecordDelivery command with signature "Bob Smith"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already delivered"

  # --- State reconstruction scenarios ---

  Scenario: Rebuild state from creation
    Given a ShipmentCreated event with order_id "ORD-050"
    When I rebuild the fulfillment state
    Then the fulfillment state has order_id "ORD-050"
    And the fulfillment state has status "pending"

  Scenario: Rebuild state to picking
    Given a ShipmentCreated event with order_id "ORD-051"
    And an ItemsPicked event
    When I rebuild the fulfillment state
    Then the fulfillment state has status "picking"

  Scenario: Rebuild state to packing
    Given a ShipmentCreated event with order_id "ORD-052"
    And an ItemsPicked event
    And an ItemsPacked event
    When I rebuild the fulfillment state
    Then the fulfillment state has status "packing"

  Scenario: Rebuild state to shipped
    Given a ShipmentCreated event with order_id "ORD-053"
    And an ItemsPicked event
    And an ItemsPacked event
    And a Shipped event
    When I rebuild the fulfillment state
    Then the fulfillment state has status "shipped"
    And the fulfillment state has tracking_number "TRACK-TEST"

  Scenario: Rebuild state to delivered
    Given a ShipmentCreated event with order_id "ORD-054"
    And an ItemsPicked event
    And an ItemsPacked event
    And a Shipped event
    And a Delivered event
    When I rebuild the fulfillment state
    Then the fulfillment state has status "delivered"
