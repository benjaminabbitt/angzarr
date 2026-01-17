Feature: Shopping Cart Business Logic
  Tests cart aggregate behavior independent of transport.
  These scenarios verify pure business logic for shopping cart operations.

  # --- CreateCart scenarios ---

  Scenario: Create a new cart for a customer
    Given no prior events for the cart aggregate
    When I handle a CreateCart command with customer_id "CUST-001"
    Then the result is a CartCreated event
    And the cart event has customer_id "CUST-001"

  Scenario: Cannot create cart twice for same customer
    Given a CartCreated event with customer_id "CUST-002"
    When I handle a CreateCart command with customer_id "CUST-002"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already exists"

  # --- AddItem scenarios ---

  Scenario: Add item to empty cart
    Given a CartCreated event with customer_id "CUST-010"
    When I handle an AddItem command with product_id "SKU-001" name "Widget" quantity 2 and unit_price_cents 1000
    Then the result is an ItemAdded event
    And the cart event has product_id "SKU-001"
    And the cart event has quantity 2
    And the cart event has new_subtotal 2000

  Scenario: Add second item to cart
    Given a CartCreated event with customer_id "CUST-011"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle an AddItem command with product_id "SKU-002" name "Gadget" quantity 1 and unit_price_cents 2500
    Then the result is an ItemAdded event
    And the cart event has new_subtotal 4500

  Scenario: Add same item increases quantity
    Given a CartCreated event with customer_id "CUST-012"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle an AddItem command with product_id "SKU-001" name "Widget" quantity 3 and unit_price_cents 1000
    Then the result is an ItemAdded event
    And the cart event has quantity 5
    And the cart event has new_subtotal 5000

  Scenario: Cannot add item to non-existent cart
    Given no prior events for the cart aggregate
    When I handle an AddItem command with product_id "SKU-001" name "Widget" quantity 1 and unit_price_cents 1000
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  Scenario: Cannot add item with zero quantity
    Given a CartCreated event with customer_id "CUST-014"
    When I handle an AddItem command with product_id "SKU-001" name "Widget" quantity 0 and unit_price_cents 1000
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "quantity"

  Scenario: Cannot add item to checked out cart
    Given a CartCreated event with customer_id "CUST-015"
    And an ItemAdded event with product_id "SKU-001" quantity 1 and unit_price_cents 1000
    And a CartCheckedOut event
    When I handle an AddItem command with product_id "SKU-002" name "Gadget" quantity 1 and unit_price_cents 2000
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "checked out"

  # --- UpdateQuantity scenarios ---

  Scenario: Update item quantity
    Given a CartCreated event with customer_id "CUST-020"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle an UpdateQuantity command with product_id "SKU-001" and new_quantity 5
    Then the result is a QuantityUpdated event
    And the cart event has new_quantity 5
    And the cart event has new_subtotal 5000

  Scenario: Cannot update quantity of item not in cart
    Given a CartCreated event with customer_id "CUST-021"
    When I handle an UpdateQuantity command with product_id "SKU-999" and new_quantity 3
    Then the command fails with status "NOT_FOUND"
    And the error message contains "not in cart"

  Scenario: Cannot set quantity to zero via update
    Given a CartCreated event with customer_id "CUST-022"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle an UpdateQuantity command with product_id "SKU-001" and new_quantity 0
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "quantity"

  # --- RemoveItem scenarios ---

  Scenario: Remove item from cart
    Given a CartCreated event with customer_id "CUST-030"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle a RemoveItem command with product_id "SKU-001"
    Then the result is an ItemRemoved event
    And the cart event has product_id "SKU-001"
    And the cart event has new_subtotal 0

  Scenario: Remove one item from multi-item cart
    Given a CartCreated event with customer_id "CUST-031"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    And an ItemAdded event with product_id "SKU-002" quantity 1 and unit_price_cents 2500
    When I handle a RemoveItem command with product_id "SKU-001"
    Then the result is an ItemRemoved event
    And the cart event has new_subtotal 2500

  Scenario: Cannot remove item not in cart
    Given a CartCreated event with customer_id "CUST-032"
    When I handle a RemoveItem command with product_id "SKU-999"
    Then the command fails with status "NOT_FOUND"
    And the error message contains "not in cart"

  # --- ApplyCoupon scenarios ---

  Scenario: Apply percentage coupon
    Given a CartCreated event with customer_id "CUST-040"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle an ApplyCoupon command with code "SAVE10" type "percentage" and value 10
    Then the result is a CouponApplied event
    And the cart event has coupon_code "SAVE10"
    And the cart event has discount_cents 200

  Scenario: Apply fixed amount coupon
    Given a CartCreated event with customer_id "CUST-041"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle an ApplyCoupon command with code "FLAT500" type "fixed" and value 500
    Then the result is a CouponApplied event
    And the cart event has coupon_code "FLAT500"
    And the cart event has discount_cents 500

  Scenario: Cannot apply coupon twice
    Given a CartCreated event with customer_id "CUST-042"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    And a CouponApplied event with code "SAVE10"
    When I handle an ApplyCoupon command with code "SAVE20" type "percentage" and value 20
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already applied"

  Scenario: Cannot apply coupon to empty cart
    Given a CartCreated event with customer_id "CUST-043"
    When I handle an ApplyCoupon command with code "SAVE10" type "percentage" and value 10
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "empty"

  # --- ClearCart scenarios ---

  Scenario: Clear cart removes all items
    Given a CartCreated event with customer_id "CUST-050"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    And an ItemAdded event with product_id "SKU-002" quantity 1 and unit_price_cents 2500
    When I handle a ClearCart command
    Then the result is a CartCleared event
    And the cart event has new_subtotal 0

  Scenario: Clear cart also removes coupon
    Given a CartCreated event with customer_id "CUST-051"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    And a CouponApplied event with code "SAVE10"
    When I handle a ClearCart command
    Then the result is a CartCleared event

  # --- Checkout scenarios ---

  Scenario: Checkout cart creates checkout event
    Given a CartCreated event with customer_id "CUST-060"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    When I handle a Checkout command
    Then the result is a CartCheckedOut event
    And the cart event has final_subtotal 2000

  Scenario: Cannot checkout empty cart
    Given a CartCreated event with customer_id "CUST-061"
    When I handle a Checkout command
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "empty"

  Scenario: Cannot checkout already checked out cart
    Given a CartCreated event with customer_id "CUST-062"
    And an ItemAdded event with product_id "SKU-001" quantity 1 and unit_price_cents 1000
    And a CartCheckedOut event
    When I handle a Checkout command
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already checked out"

  # --- State reconstruction scenarios ---

  Scenario: Rebuild state from creation
    Given a CartCreated event with customer_id "CUST-070"
    When I rebuild the cart state
    Then the cart state has customer_id "CUST-070"
    And the cart state has 0 items
    And the cart state has subtotal 0
    And the cart state has status "active"

  Scenario: Rebuild state with items
    Given a CartCreated event with customer_id "CUST-071"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    And an ItemAdded event with product_id "SKU-002" quantity 1 and unit_price_cents 2500
    When I rebuild the cart state
    Then the cart state has 2 items
    And the cart state has subtotal 4500

  Scenario: Rebuild state with coupon
    Given a CartCreated event with customer_id "CUST-072"
    And an ItemAdded event with product_id "SKU-001" quantity 2 and unit_price_cents 1000
    And a CouponApplied event with code "SAVE10"
    When I rebuild the cart state
    Then the cart state has coupon_code "SAVE10"
    And the cart state has discount_cents 200

  Scenario: Rebuild state to checked out
    Given a CartCreated event with customer_id "CUST-073"
    And an ItemAdded event with product_id "SKU-001" quantity 1 and unit_price_cents 1000
    And a CartCheckedOut event
    When I rebuild the cart state
    Then the cart state has status "checked_out"
