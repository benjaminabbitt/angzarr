Feature: Order Discount Calculator
  As an e-commerce system
  I want to calculate and apply discounts to orders
  So that customers receive appropriate pricing

  Background:
    Given an empty event store

  @python
  Scenario Outline: Apply percentage discount for orders over threshold (Python)
    Given Python business logic from module "discount_logic" at path "examples/python"
    And prior events for aggregate "<aggregate>" in domain "discounts":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
      | 2        | ItemAdded    |
    When I send an "ApplyPercentageDiscount" command via Python for aggregate "<aggregate>" in domain "discounts"
    Then 4 events total exist for aggregate "<aggregate>"
    And the latest event type contains "DiscountApplied"
    And the event bus receives the new events

    Examples:
      | aggregate      |
      | py-discount-01 |

  @go-ffi
  Scenario Outline: Apply percentage discount for orders over threshold (Go)
    Given Go business logic from library "examples/golang/libbusiness.so"
    And prior events for aggregate "<aggregate>" in domain "discounts":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
      | 2        | ItemAdded    |
    When I send an "ApplyPercentageDiscount" command via Go for aggregate "<aggregate>" in domain "discounts"
    Then 4 events total exist for aggregate "<aggregate>"
    And the latest event type contains "DiscountApplied"
    And the event bus receives the new events

    Examples:
      | aggregate      |
      | go-discount-01 |

  @python
  Scenario Outline: Reject discount on empty order (Python)
    Given Python business logic from module "discount_logic" at path "examples/python"
    And no prior events for aggregate "<aggregate>" in domain "discounts"
    When I send an "ApplyPercentageDiscount" command via Python for aggregate "<aggregate>" in domain "discounts"
    Then the command is rejected with error containing "no order"

    Examples:
      | aggregate      |
      | py-discount-02 |

  @go-ffi
  Scenario Outline: Reject discount on empty order (Go)
    Given Go business logic from library "examples/golang/libbusiness.so"
    And no prior events for aggregate "<aggregate>" in domain "discounts"
    When I send an "ApplyPercentageDiscount" command via Go for aggregate "<aggregate>" in domain "discounts"
    Then the command is rejected with error containing "no order"

    Examples:
      | aggregate      |
      | go-discount-02 |

  @python
  Scenario Outline: Apply coupon code discount (Python)
    Given Python business logic from module "discount_logic" at path "examples/python"
    And prior events for aggregate "<aggregate>" in domain "discounts":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
    When I send an "ApplyCoupon" command via Python for aggregate "<aggregate>" in domain "discounts"
    Then 3 events total exist for aggregate "<aggregate>"
    And the latest event type contains "CouponApplied"
    And the event bus receives the new events

    Examples:
      | aggregate      |
      | py-discount-03 |

  @go-ffi
  Scenario Outline: Apply coupon code discount (Go)
    Given Go business logic from library "examples/golang/libbusiness.so"
    And prior events for aggregate "<aggregate>" in domain "discounts":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
    When I send an "ApplyCoupon" command via Go for aggregate "<aggregate>" in domain "discounts"
    Then 3 events total exist for aggregate "<aggregate>"
    And the latest event type contains "CouponApplied"
    And the event bus receives the new events

    Examples:
      | aggregate      |
      | go-discount-03 |

  @python
  Scenario Outline: Cannot stack multiple percentage discounts (Python)
    Given Python business logic from module "discount_logic" at path "examples/python"
    And prior events for aggregate "<aggregate>" in domain "discounts":
      | sequence | event_type      |
      | 0        | OrderCreated    |
      | 1        | ItemAdded       |
      | 2        | DiscountApplied |
    When I send an "ApplyPercentageDiscount" command via Python for aggregate "<aggregate>" in domain "discounts"
    Then the command is rejected with error containing "already has a discount"

    Examples:
      | aggregate      |
      | py-discount-04 |

  @python
  Scenario Outline: Remove discount from order (Python)
    Given Python business logic from module "discount_logic" at path "examples/python"
    And prior events for aggregate "<aggregate>" in domain "discounts":
      | sequence | event_type      |
      | 0        | OrderCreated    |
      | 1        | ItemAdded       |
      | 2        | DiscountApplied |
    When I send a "RemoveDiscount" command via Python for aggregate "<aggregate>" in domain "discounts"
    Then 4 events total exist for aggregate "<aggregate>"
    And the latest event type contains "DiscountRemoved"
    And the event bus receives the new events

    Examples:
      | aggregate      |
      | py-discount-05 |

  @python
  Scenario Outline: Calculate bulk discount for large quantities (Python)
    Given Python business logic from module "discount_logic" at path "examples/python"
    And prior events for aggregate "<aggregate>" in domain "discounts":
      | sequence | event_type   |
      | 0        | OrderCreated |
      | 1        | ItemAdded    |
      | 2        | ItemAdded    |
      | 3        | ItemAdded    |
      | 4        | ItemAdded    |
      | 5        | ItemAdded    |
    When I send a "CalculateBulkDiscount" command via Python for aggregate "<aggregate>" in domain "discounts"
    Then 7 events total exist for aggregate "<aggregate>"
    And the latest event type contains "BulkDiscountCalculated"
    And the event bus receives the new events

    Examples:
      | aggregate      |
      | py-discount-06 |
