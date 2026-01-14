Feature: Log Projector Logic
  Tests log projector behavior independent of transport.
  Log projectors output structured logs for events.

  # Note: Log projectors are simple - they just log events.
  # These tests verify the events are processed without errors.

  Scenario: Log CustomerCreated event
    Given a CustomerCreated event with name "Alice" and email "alice@example.com"
    When I process the log projector
    Then the event is logged successfully

  Scenario: Log LoyaltyPointsAdded event
    Given a LoyaltyPointsAdded event with 100 points and new_balance 100
    When I process the log projector
    Then the event is logged successfully

  Scenario: Log TransactionCreated event
    Given a TransactionCreated event with customer "cust-001" and subtotal 2000
    When I process the log projector
    Then the event is logged successfully

  Scenario: Log TransactionCompleted event
    Given a TransactionCompleted event with total 2000 and payment "card"
    When I process the log projector
    Then the event is logged successfully

  Scenario: Handle unknown event type gracefully
    Given an unknown event type
    When I process the log projector
    Then the event is logged as unknown
