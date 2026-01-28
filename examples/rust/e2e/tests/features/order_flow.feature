Feature: Complete Order Lifecycle
  Tests the full business flow from cart creation through fulfillment,
  validating correlation ID propagation across all domains.

  Background:
    # Tests run against standalone mode with SQLite storage

  # ===========================================================================
  # Cart Operations
  # ===========================================================================

  @e2e @flow @cart
  Scenario: Create a shopping cart
    When I create a cart "CART-001" with correlation "CORR-001"
    Then the command succeeds
    And an event "CartCreated" is emitted
    And the correlation ID "CORR-001" is in the response

  @e2e @flow @cart
  Scenario: Add items to cart
    Given a cart "CART-002" exists
    When I add item "WIDGET-001" quantity 2 to cart "CART-002"
    Then the command succeeds
    And an event "ItemAdded" is emitted
    And the cart "CART-002" has 1 line item

  @e2e @flow @cart
  Scenario: Update item quantity
    Given a cart "CART-003" with item "WIDGET-001" quantity 1
    When I update item "WIDGET-001" to quantity 5 in cart "CART-003"
    Then the command succeeds
    And an event "QuantityUpdated" is emitted
    And the cart "CART-003" item "WIDGET-001" has quantity 5

  @e2e @flow @cart
  Scenario: Remove item from cart
    Given a cart "CART-004" with item "WIDGET-001" quantity 3
    When I remove item "WIDGET-001" from cart "CART-004"
    Then the command succeeds
    And an event "ItemRemoved" is emitted
    And the cart "CART-004" has 0 line items

  @e2e @flow @cart
  Scenario: Apply coupon to cart
    Given a cart "CART-005" with item "WIDGET-001" quantity 1
    When I apply coupon "SAVE10" to cart "CART-005"
    Then the command succeeds
    And an event "CouponApplied" is emitted

  @e2e @flow @cart
  Scenario: Clear cart
    Given a cart "CART-006" with item "WIDGET-001" quantity 2
    When I clear cart "CART-006"
    Then the command succeeds
    And an event "CartCleared" is emitted
    And the cart "CART-006" has 0 line items

  # ===========================================================================
  # Checkout Flow
  # ===========================================================================

  @e2e @flow @checkout
  Scenario: Checkout cart creates order
    Given a cart "CART-CHECKOUT" with items:
      | sku        | quantity |
      | WIDGET-001 | 2        |
      | WIDGET-002 | 1        |
    When I checkout cart "CART-CHECKOUT" with correlation "ORDER-CORR-001"
    Then the command succeeds
    And an event "CheckedOut" is emitted
    And the correlation ID "ORDER-CORR-001" is preserved

  # ===========================================================================
  # Full Business Flow
  # ===========================================================================

  @e2e @flow @full
  Scenario: Complete order lifecycle with correlation tracking
    # Cart phase
    When I create a cart "FULL-CART" with correlation "FULL-CORR"
    Then the command succeeds

    When I add item "SKU-FULL-001" quantity 3 to cart "FULL-CART"
    Then the command succeeds

    When I add item "SKU-FULL-002" quantity 1 to cart "FULL-CART"
    Then the command succeeds

    # Verify cart state
    Then the cart "FULL-CART" has 2 line items

    # Apply discount
    When I apply coupon "LOYALTY20" to cart "FULL-CART"
    Then the command succeeds

    # Checkout
    When I checkout cart "FULL-CART" with correlation "FULL-CORR"
    Then the command succeeds

    # Verify correlation ID appears in all events
    Then correlation "FULL-CORR" appears in events:
      | event_type      |
      | CartCreated     |
      | ItemAdded       |
      | CouponApplied   |
      | CheckedOut      |

  @e2e @flow @full
  Scenario: Multiple carts with independent correlation IDs
    # First cart
    When I create a cart "MULTI-CART-1" with correlation "MULTI-CORR-1"
    And I add item "SKU-001" quantity 1 to cart "MULTI-CART-1"

    # Second cart (different correlation)
    When I create a cart "MULTI-CART-2" with correlation "MULTI-CORR-2"
    And I add item "SKU-002" quantity 2 to cart "MULTI-CART-2"

    # Verify isolation
    Then correlation "MULTI-CORR-1" only appears in cart "MULTI-CART-1" events
    And correlation "MULTI-CORR-2" only appears in cart "MULTI-CART-2" events

  # ===========================================================================
  # Error Handling
  # ===========================================================================

  @e2e @flow @errors
  Scenario: Add item to non-existent cart fails
    Given no cart exists for "GHOST-CART"
    When I add item "SKU-001" quantity 1 to cart "GHOST-CART" expecting sequence 5
    Then the command fails with "FailedPrecondition"

  @e2e @flow @errors
  Scenario: Checkout empty cart fails
    Given an empty cart "EMPTY-CART" exists
    When I checkout cart "EMPTY-CART"
    Then the command fails with "cart is empty"

  @e2e @flow @errors
  Scenario: Remove non-existent item fails
    Given a cart "CART-NO-ITEM" with item "WIDGET-001" quantity 1
    When I remove item "GHOST-ITEM" from cart "CART-NO-ITEM"
    Then the command fails with "Item not in cart"
