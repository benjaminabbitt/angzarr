Feature: Edition — Diverged Event Timelines
  Editions allow diverging from the main timeline at a point in history,
  creating an independent branch that shares events up to divergence
  and then continues independently. No data is copied.

  Background:
    Given a product "WIDGET" exists with price 1000 cents
    And inventory for "WIDGET" has 100 units
    And customer "Alice" exists with 500 loyalty points

  Scenario: Create an edition and execute a command on it
    Given a cart "ed-cart-1" with item "WIDGET" quantity 2
    When I create edition "v2" diverging at sequence 2 for domain "cart"
    And I execute on edition "v2" an AddItem for cart "ed-cart-1" with product "WIDGET" quantity 3
    Then edition "v2" should have 3 events for domain "cart" root "ed-cart-1"
    And the main timeline should have 2 events for domain "cart" root "ed-cart-1"

  Scenario: Edition reads include main timeline history up to divergence
    Given a cart "ed-cart-2" with item "WIDGET" quantity 1
    When I create edition "branch" diverging at sequence 2 for domain "cart"
    Then edition "branch" should have 2 events for domain "cart" root "ed-cart-2"

  Scenario: Main timeline is unaffected by edition commands
    Given a cart "ed-cart-3" exists
    When I create edition "isolated" diverging at sequence 1 for domain "cart"
    And I execute on edition "isolated" an AddItem for cart "ed-cart-3" with product "WIDGET" quantity 5
    Then the main timeline should have 1 events for domain "cart" root "ed-cart-3"

  Scenario: List and delete editions
    When I create edition "alpha" diverging at sequence 0 for domain "cart"
    And I create edition "beta" diverging at sequence 0 for domain "cart"
    Then listing editions should show 2 active editions
    When I delete edition "alpha"
    Then listing editions should show 1 active editions

  Scenario: Duplicate edition name is rejected
    When I create edition "unique" diverging at sequence 0 for domain "cart"
    And I try to create edition "unique" diverging at sequence 0 for domain "cart"
    Then the edition creation should fail with "already exists"
