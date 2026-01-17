Feature: Order Business Logic
  Tests order aggregate behavior independent of transport.
  These scenarios verify pure business logic for order lifecycle and payment.

  # --- CreateOrder scenarios ---

  Scenario: Create a new order with items
    Given no prior events for the order aggregate
    When I handle a CreateOrder command with customer_id "CUST-001" and items:
      | product_id | name     | quantity | unit_price_cents |
      | SKU-001    | Widget   | 2        | 1000             |
      | SKU-002    | Gadget   | 1        | 2500             |
    Then the result is an OrderCreated event
    And the order event has customer_id "CUST-001"
    And the order event has 2 items
    And the order event has subtotal_cents 4500

  Scenario: Cannot create order twice
    Given an OrderCreated event with customer_id "CUST-002" and subtotal 3000
    When I handle a CreateOrder command with customer_id "CUST-002" and items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-003    | Item   | 1        | 1000             |
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already exists"

  Scenario: Cannot create order without items
    Given no prior events for the order aggregate
    When I handle a CreateOrder command with customer_id "CUST-003" and no items
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "items"

  Scenario: Cannot create order with invalid quantity
    Given no prior events for the order aggregate
    When I handle a CreateOrder command with customer_id "CUST-004" and items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-001    | Widget | 0        | 1000             |
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "quantity"

  # --- ApplyLoyaltyDiscount scenarios ---

  Scenario: Apply loyalty points discount
    Given an OrderCreated event with customer_id "CUST-010" and subtotal 5000
    When I handle an ApplyLoyaltyDiscount command with points 200 worth 200 cents
    Then the result is a LoyaltyDiscountApplied event
    And the order event has points_used 200
    And the order event has discount_cents 200

  Scenario: Cannot apply loyalty discount twice
    Given an OrderCreated event with customer_id "CUST-011" and subtotal 5000
    And a LoyaltyDiscountApplied event with 100 points
    When I handle an ApplyLoyaltyDiscount command with points 50 worth 50 cents
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already applied"

  Scenario: Cannot apply loyalty discount to non-existent order
    Given no prior events for the order aggregate
    When I handle an ApplyLoyaltyDiscount command with points 100 worth 100 cents
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  # --- SubmitPayment scenarios ---

  Scenario: Submit payment for order
    Given an OrderCreated event with customer_id "CUST-020" and subtotal 5000
    When I handle a SubmitPayment command with method "card" and amount 5000 cents
    Then the result is a PaymentSubmitted event
    And the order event has payment_method "card"
    And the order event has amount_cents 5000

  Scenario: Submit payment with loyalty discount applied
    Given an OrderCreated event with customer_id "CUST-021" and subtotal 5000
    And a LoyaltyDiscountApplied event with 500 points
    When I handle a SubmitPayment command with method "card" and amount 4500 cents
    Then the result is a PaymentSubmitted event
    And the order event has amount_cents 4500

  Scenario: Cannot submit payment for incorrect amount
    Given an OrderCreated event with customer_id "CUST-022" and subtotal 5000
    When I handle a SubmitPayment command with method "card" and amount 4000 cents
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "amount"

  Scenario: Cannot submit payment twice
    Given an OrderCreated event with customer_id "CUST-023" and subtotal 5000
    And a PaymentSubmitted event
    When I handle a SubmitPayment command with method "card" and amount 5000 cents
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already submitted"

  # --- ConfirmPayment scenarios ---

  Scenario: Confirm payment completes the order
    Given an OrderCreated event with customer_id "CUST-030" and subtotal 5000
    And a PaymentSubmitted event
    When I handle a ConfirmPayment command with reference "PAY-12345"
    Then the result is an OrderCompleted event
    And the order event has final_total_cents 5000
    And the order event has loyalty_points_earned 50

  Scenario: Cannot confirm payment without submission
    Given an OrderCreated event with customer_id "CUST-031" and subtotal 5000
    When I handle a ConfirmPayment command with reference "PAY-99999"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "payment not submitted"

  Scenario: Cannot confirm already completed order
    Given an OrderCreated event with customer_id "CUST-032" and subtotal 5000
    And a PaymentSubmitted event
    And an OrderCompleted event
    When I handle a ConfirmPayment command with reference "PAY-DOUBLE"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already completed"

  # --- CancelOrder scenarios ---

  Scenario: Cancel a pending order
    Given an OrderCreated event with customer_id "CUST-040" and subtotal 5000
    When I handle a CancelOrder command with reason "Customer request"
    Then the result is an OrderCancelled event
    And the order event has reason "Customer request"

  Scenario: Cancel order after payment submitted
    Given an OrderCreated event with customer_id "CUST-041" and subtotal 5000
    And a PaymentSubmitted event
    When I handle a CancelOrder command with reason "Payment failed"
    Then the result is an OrderCancelled event

  Scenario: Cannot cancel completed order
    Given an OrderCreated event with customer_id "CUST-042" and subtotal 5000
    And a PaymentSubmitted event
    And an OrderCompleted event
    When I handle a CancelOrder command with reason "Too late"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "completed"

  Scenario: Cannot cancel already cancelled order
    Given an OrderCreated event with customer_id "CUST-043" and subtotal 5000
    And an OrderCancelled event
    When I handle a CancelOrder command with reason "Double cancel"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already cancelled"

  # --- State reconstruction scenarios ---

  Scenario: Rebuild state from creation
    Given an OrderCreated event with customer_id "CUST-050" and subtotal 5000
    When I rebuild the order state
    Then the order state has customer_id "CUST-050"
    And the order state has subtotal_cents 5000
    And the order state has status "pending"

  Scenario: Rebuild state with loyalty discount
    Given an OrderCreated event with customer_id "CUST-051" and subtotal 5000
    And a LoyaltyDiscountApplied event with 200 points
    When I rebuild the order state
    Then the order state has loyalty_points_used 200
    And the order state has discount_cents 200

  Scenario: Rebuild state to completed
    Given an OrderCreated event with customer_id "CUST-052" and subtotal 5000
    And a PaymentSubmitted event
    And an OrderCompleted event
    When I rebuild the order state
    Then the order state has status "completed"

  Scenario: Rebuild state to cancelled
    Given an OrderCreated event with customer_id "CUST-053" and subtotal 5000
    And an OrderCancelled event
    When I rebuild the order state
    Then the order state has status "cancelled"
