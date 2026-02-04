Feature: Loyalty Earn Saga
  Tests the saga that awards loyalty points when an order is completed.
  Listens to OrderCompleted events and generates AddLoyaltyPoints commands.

  Scenario: Award loyalty points when order completes
    Given an OrderCompleted event with loyalty_points_earned 50 for customer "CUST-001"
    When I process the loyalty earn saga
    Then an AddLoyaltyPoints command is generated
    And the command targets "customer" domain
    And the command has points 50
    And the command has reason containing "order"

  Scenario: No command generated for zero points
    Given an OrderCompleted event with loyalty_points_earned 0 for customer "CUST-002"
    When I process the loyalty earn saga
    Then no commands are generated

  Scenario: Ignore non-OrderCompleted events
    Given an OrderCreated event for customer "CUST-003"
    When I process the loyalty earn saga
    Then no commands are generated

  Scenario: Preserve correlation ID
    Given an OrderCompleted event with loyalty_points_earned 25 for customer "CUST-004"
    And the correlation_id is "CORR-12345"
    When I process the loyalty earn saga
    Then the command has correlation_id "CORR-12345"
