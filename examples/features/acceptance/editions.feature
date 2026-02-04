@standalone
Feature: Edition — Diverged Event Timelines
  Editions allow diverging from the main timeline at a point in history,
  creating an independent branch that shares events up to divergence
  and then continues independently. No data is copied.

  Background:
    Given inventory for "WIDGET" has 100 units

  Scenario: Create an edition and execute a command on it
    Given an order "ed-order-1" with item "WIDGET" quantity 2
    When I create edition "v2" diverging at sequence 1 for domain "order"
    And I execute on edition "v2" a SubmitPayment for order "ed-order-1" with amount 5000
    Then edition "v2" should have 2 events for domain "order" root "ed-order-1"
    And the main timeline should have 1 events for domain "order" root "ed-order-1"

  Scenario: Edition reads include main timeline history up to divergence
    Given an order "ed-order-2" with item "WIDGET" quantity 1
    And payment submitted for order "ed-order-2"
    When I create edition "branch" diverging at sequence 2 for domain "order"
    Then edition "branch" should have 2 events for domain "order" root "ed-order-2"

  Scenario: Main timeline is unaffected by edition commands
    Given an order "ed-order-3" exists
    When I create edition "isolated" diverging at sequence 1 for domain "order"
    And I execute on edition "isolated" a SubmitPayment for order "ed-order-3" with amount 3000
    Then the main timeline should have 1 events for domain "order" root "ed-order-3"

  Scenario: List and delete editions
    When I create edition "alpha" diverging at sequence 0 for domain "order"
    And I create edition "beta" diverging at sequence 0 for domain "order"
    Then listing editions should show 2 active editions
    When I delete edition "alpha"
    Then listing editions should show 1 active editions

  Scenario: Duplicate edition name is rejected
    When I create edition "unique" diverging at sequence 0 for domain "order"
    And I try to create edition "unique" diverging at sequence 0 for domain "order"
    Then the edition creation should fail with "already exists"
