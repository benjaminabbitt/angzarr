Feature: Receipt Projector Logic
  Tests receipt projector behavior independent of transport.
  The receipt projector generates receipt documents from completed transactions.

  Scenario: No projection for incomplete transaction
    Given a TransactionCreated event with customer "cust-001" and subtotal 2000
    When I project the events
    Then no projection is generated

  Scenario: Generate receipt for completed transaction
    Given a TransactionCreated event with customer "cust-001" and items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-001    | Widget | 2        | 1000             |
    And a TransactionCompleted event with total 2000 and payment "card"
    When I project the events
    Then a Receipt projection is generated
    And the receipt has customer_id "cust-001"
    And the receipt has final_total_cents 2000
    And the receipt has payment_method "card"

  Scenario: Receipt includes discount
    Given a TransactionCreated event with customer "cust-002" and subtotal 2000
    And a DiscountApplied event with 200 cents discount
    And a TransactionCompleted event with total 1800 and payment "cash"
    When I project the events
    Then a Receipt projection is generated
    And the receipt has subtotal_cents 2000
    And the receipt has discount_cents 200
    And the receipt has final_total_cents 1800

  Scenario: Receipt includes loyalty points earned
    Given a TransactionCreated event with customer "cust-003" and subtotal 5000
    And a TransactionCompleted event with total 5000 and payment "card" earning 50 points
    When I project the events
    Then a Receipt projection is generated
    And the receipt has loyalty_points_earned 50

  Scenario: Receipt has formatted text
    Given a TransactionCreated event with customer "cust-004" and items:
      | product_id | name   | quantity | unit_price_cents |
      | SKU-001    | Widget | 1        | 1000             |
    And a TransactionCompleted event with total 1000 and payment "card"
    When I project the events
    Then a Receipt projection is generated
    And the receipt formatted_text contains "RECEIPT"
    And the receipt formatted_text contains "Widget"
    And the receipt formatted_text contains "Thank you"
