Feature: Customer Business Logic
  Tests customer aggregate behavior independent of transport.
  These scenarios verify pure business logic for customer lifecycle and loyalty points.

  # --- CreateCustomer scenarios ---

  Scenario: Create a new customer
    Given no prior events for the aggregate
    When I handle a CreateCustomer command with name "Alice" and email "alice@example.com"
    Then the result is a CustomerCreated event
    And the event has name "Alice"
    And the event has email "alice@example.com"

  Scenario: Cannot create customer with same email
    Given a CustomerCreated event with name "Bob" and email "bob@example.com"
    When I handle a CreateCustomer command with name "Robert" and email "bob@example.com"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already exists"

  Scenario: Creating customer requires name
    Given no prior events for the aggregate
    When I handle a CreateCustomer command with name "" and email "noname@example.com"
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "name"

  Scenario: Creating customer requires email
    Given no prior events for the aggregate
    When I handle a CreateCustomer command with name "NoEmail" and email ""
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "email"

  # --- AddLoyaltyPoints scenarios ---

  Scenario: Add loyalty points to existing customer
    Given a CustomerCreated event with name "Carol" and email "carol@example.com"
    When I handle an AddLoyaltyPoints command with 100 points and reason "signup bonus"
    Then the result is a LoyaltyPointsAdded event
    And the event has points 100
    And the event has new_balance 100
    And the event has reason "signup bonus"

  Scenario: Add loyalty points accumulates balance
    Given a CustomerCreated event with name "Dave" and email "dave@example.com"
    And a LoyaltyPointsAdded event with 50 points and new_balance 50
    When I handle an AddLoyaltyPoints command with 30 points and reason "purchase"
    Then the result is a LoyaltyPointsAdded event
    And the event has new_balance 80

  Scenario: Cannot add points to non-existent customer
    Given no prior events for the aggregate
    When I handle an AddLoyaltyPoints command with 100 points and reason "test"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  Scenario: Cannot add zero points
    Given a CustomerCreated event with name "Eve" and email "eve@example.com"
    When I handle an AddLoyaltyPoints command with 0 points and reason "invalid"
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "positive"

  Scenario: Cannot add negative points
    Given a CustomerCreated event with name "Frank" and email "frank@example.com"
    When I handle an AddLoyaltyPoints command with -10 points and reason "invalid"
    Then the command fails with status "INVALID_ARGUMENT"

  # --- RedeemLoyaltyPoints scenarios ---

  Scenario: Redeem loyalty points
    Given a CustomerCreated event with name "Grace" and email "grace@example.com"
    And a LoyaltyPointsAdded event with 100 points and new_balance 100
    When I handle a RedeemLoyaltyPoints command with 50 points and type "discount"
    Then the result is a LoyaltyPointsRedeemed event
    And the event has points 50
    And the event has new_balance 50
    And the event has redemption_type "discount"

  Scenario: Cannot redeem more points than available
    Given a CustomerCreated event with name "Henry" and email "henry@example.com"
    And a LoyaltyPointsAdded event with 50 points and new_balance 50
    When I handle a RedeemLoyaltyPoints command with 100 points and type "discount"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "Insufficient points"

  Scenario: Cannot redeem points from non-existent customer
    Given no prior events for the aggregate
    When I handle a RedeemLoyaltyPoints command with 50 points and type "discount"
    Then the command fails with status "FAILED_PRECONDITION"

  Scenario: Cannot redeem zero points
    Given a CustomerCreated event with name "Ivy" and email "ivy@example.com"
    And a LoyaltyPointsAdded event with 100 points and new_balance 100
    When I handle a RedeemLoyaltyPoints command with 0 points and type "invalid"
    Then the command fails with status "INVALID_ARGUMENT"

  # --- State reconstruction scenarios ---

  Scenario: Rebuild state from multiple events
    Given a CustomerCreated event with name "Jack" and email "jack@example.com"
    And a LoyaltyPointsAdded event with 100 points and new_balance 100
    And a LoyaltyPointsAdded event with 50 points and new_balance 150
    And a LoyaltyPointsRedeemed event with 30 points and new_balance 120
    When I rebuild the customer state
    Then the state has name "Jack"
    And the state has email "jack@example.com"
    And the state has loyalty_points 120
    And the state has lifetime_points 150

  Scenario: Lifetime points not reduced by redemptions
    Given a CustomerCreated event with name "Kate" and email "kate@example.com"
    And a LoyaltyPointsAdded event with 200 points and new_balance 200
    And a LoyaltyPointsRedeemed event with 50 points and new_balance 150
    When I rebuild the customer state
    Then the state has loyalty_points 150
    And the state has lifetime_points 200
