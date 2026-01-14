Feature: Loyalty Saga Logic
  Tests loyalty saga behavior independent of transport.
  The loyalty saga awards loyalty points when transactions complete.

  Scenario: No commands for incomplete transaction
    Given a TransactionCreated event with customer "cust-001" and subtotal 2000
    When I process the saga
    Then no commands are generated

  Scenario: Generate AddLoyaltyPoints command for completed transaction
    Given a TransactionCompleted event with 20 loyalty points earned
    When I process the saga
    Then an AddLoyaltyPoints command is generated
    And the command has points 20
    And the command has domain "customer"

  Scenario: No command for zero points
    Given a TransactionCompleted event with 0 loyalty points earned
    When I process the saga
    Then no commands are generated

  Scenario: Command includes transaction reference in reason
    Given a TransactionCompleted event with 50 loyalty points earned
    When I process the saga
    Then an AddLoyaltyPoints command is generated
    And the command reason contains "transaction"
