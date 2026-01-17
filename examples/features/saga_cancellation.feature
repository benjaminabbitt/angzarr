Feature: Order Cancellation Saga
  Tests the saga that handles order cancellation compensation.
  Listens to OrderCancelled events and generates compensation commands.

  Scenario: Release inventory when order cancelled
    Given an OrderCancelled event for order "ORD-001" with reason "Customer request"
    When I process the cancellation saga
    Then a ReleaseReservation command is generated
    And the command targets "inventory" domain
    And the command has order_id "ORD-001"

  Scenario: Reverse loyalty points when order cancelled with points used
    Given an OrderCancelled event for order "ORD-002" with loyalty_points_used 100
    When I process the cancellation saga
    Then commands are generated for "inventory" and "customer" domains

  Scenario: No customer command when no points used
    Given an OrderCancelled event for order "ORD-003" with loyalty_points_used 0
    When I process the cancellation saga
    Then only an inventory command is generated

  Scenario: Ignore non-cancelled events
    Given an OrderCreated event for order "ORD-004"
    When I process the cancellation saga
    Then no commands are generated

  Scenario: Preserve correlation ID in all commands
    Given an OrderCancelled event for order "ORD-005" with loyalty_points_used 50
    And the correlation_id is "CORR-CANCEL-001"
    When I process the cancellation saga
    Then all commands have correlation_id "CORR-CANCEL-001"
