Feature: Transaction Business Logic
  Tests transaction aggregate behavior independent of transport.
  These scenarios verify pure business logic for transaction lifecycle.

  # --- CreateTransaction scenarios ---

  Scenario: Create a new transaction
    Given no prior events for the aggregate
    When I handle a CreateTransaction command with customer "cust-001" and items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-001    | Widget | 2        | 1000             |
    Then the result is a TransactionCreated event
    And the event has customer_id "cust-001"
    And the event has subtotal_cents 2000

  Scenario: Create transaction with multiple items
    Given no prior events for the aggregate
    When I handle a CreateTransaction command with customer "cust-002" and items:
      | product_id | name     | quantity | unit_price_cents |
      | SKU-001    | Widget   | 2        | 1000             |
      | SKU-002    | Gadget   | 1        | 2500             |
    Then the result is a TransactionCreated event
    And the event has subtotal_cents 4500

  Scenario: Cannot create transaction twice
    Given a TransactionCreated event with customer "cust-003" and subtotal 3000
    When I handle a CreateTransaction command with customer "cust-003" and items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-001    | Widget | 1        | 1000             |
    Then the command fails with status "FAILED_PRECONDITION"

  Scenario: Creating transaction requires customer ID
    Given no prior events for the aggregate
    When I handle a CreateTransaction command with customer "" and items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-001    | Widget | 1        | 1000             |
    Then the command fails with status "INVALID_ARGUMENT"

  Scenario: Creating transaction requires items
    Given no prior events for the aggregate
    When I handle a CreateTransaction command with customer "cust-004" and no items
    Then the command fails with status "INVALID_ARGUMENT"

  # --- ApplyDiscount scenarios ---

  Scenario: Apply percentage discount
    Given a TransactionCreated event with customer "cust-005" and subtotal 2000
    When I handle an ApplyDiscount command with type "percentage" and value 10
    Then the result is a DiscountApplied event
    And the event has discount_cents 200

  Scenario: Apply fixed discount
    Given a TransactionCreated event with customer "cust-006" and subtotal 2000
    When I handle an ApplyDiscount command with type "fixed" and value 500
    Then the result is a DiscountApplied event
    And the event has discount_cents 500

  Scenario: Cannot apply discount to non-pending transaction
    Given a TransactionCreated event with customer "cust-007" and subtotal 2000
    And a TransactionCompleted event
    When I handle an ApplyDiscount command with type "percentage" and value 10
    Then the command fails with status "FAILED_PRECONDITION"

  # --- CompleteTransaction scenarios ---

  Scenario: Complete transaction
    Given a TransactionCreated event with customer "cust-008" and subtotal 2000
    When I handle a CompleteTransaction command with payment method "card"
    Then the result is a TransactionCompleted event
    And the event has final_total_cents 2000
    And the event has payment_method "card"
    And the event has loyalty_points_earned 20

  Scenario: Complete transaction with discount
    Given a TransactionCreated event with customer "cust-009" and subtotal 2000
    And a DiscountApplied event with 200 cents discount
    When I handle a CompleteTransaction command with payment method "cash"
    Then the result is a TransactionCompleted event
    And the event has final_total_cents 1800
    And the event has loyalty_points_earned 18

  Scenario: Cannot complete non-pending transaction
    Given no prior events for the aggregate
    When I handle a CompleteTransaction command with payment method "card"
    Then the command fails with status "FAILED_PRECONDITION"

  # --- CancelTransaction scenarios ---

  Scenario: Cancel transaction
    Given a TransactionCreated event with customer "cust-010" and subtotal 2000
    When I handle a CancelTransaction command with reason "customer request"
    Then the result is a TransactionCancelled event
    And the event has reason "customer request"

  Scenario: Cannot cancel completed transaction
    Given a TransactionCreated event with customer "cust-011" and subtotal 2000
    And a TransactionCompleted event
    When I handle a CancelTransaction command with reason "too late"
    Then the command fails with status "FAILED_PRECONDITION"

  # --- State reconstruction scenarios ---

  Scenario: Rebuild pending transaction state
    Given a TransactionCreated event with customer "cust-012" and subtotal 3000
    When I rebuild the transaction state
    Then the state has customer_id "cust-012"
    And the state has subtotal_cents 3000
    And the state has status "pending"

  Scenario: Rebuild completed transaction state
    Given a TransactionCreated event with customer "cust-013" and subtotal 3000
    And a TransactionCompleted event
    When I rebuild the transaction state
    Then the state has status "completed"
