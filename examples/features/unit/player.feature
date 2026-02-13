Feature: Player aggregate logic
  Tests player aggregate behavior for bankroll management and table reservations.

  # --- RegisterPlayer scenarios ---

  Scenario: Register a new human player
    Given no prior events for the player aggregate
    When I handle a RegisterPlayer command with name "Alice" and email "alice@example.com"
    Then the result is a PlayerRegistered event
    And the player event has display_name "Alice"
    And the player event has player_type "HUMAN"

  Scenario: Register an AI player
    Given no prior events for the player aggregate
    When I handle a RegisterPlayer command with name "Bot1" and email "bot1@example.com" as AI
    Then the result is a PlayerRegistered event
    And the player event has player_type "AI"

  Scenario: Cannot register player twice
    Given a PlayerRegistered event for "Alice"
    When I handle a RegisterPlayer command with name "Alice2" and email "alice@example.com"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already exists"

  # --- DepositFunds scenarios ---

  Scenario: Deposit funds to bankroll
    Given a PlayerRegistered event for "Alice"
    When I handle a DepositFunds command with amount 1000
    Then the result is a FundsDeposited event
    And the player event has amount 1000
    And the player event has new_balance 1000

  Scenario: Multiple deposits accumulate
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 500
    When I handle a DepositFunds command with amount 300
    Then the result is a FundsDeposited event
    And the player event has new_balance 800

  Scenario: Cannot deposit to non-existent player
    Given no prior events for the player aggregate
    When I handle a DepositFunds command with amount 1000
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "does not exist"

  Scenario: Cannot deposit zero or negative
    Given a PlayerRegistered event for "Alice"
    When I handle a DepositFunds command with amount 0
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "positive"

  # --- WithdrawFunds scenarios ---

  Scenario: Withdraw funds from bankroll
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 1000
    When I handle a WithdrawFunds command with amount 400
    Then the result is a FundsWithdrawn event
    And the player event has amount 400
    And the player event has new_balance 600

  Scenario: Cannot withdraw more than available
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 500
    When I handle a WithdrawFunds command with amount 600
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "insufficient"

  Scenario: Cannot withdraw with funds reserved
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 1000
    And a FundsReserved event with amount 800 for table "table-1"
    When I handle a WithdrawFunds command with amount 300
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "insufficient"

  # --- ReserveFunds scenarios ---

  Scenario: Reserve funds for table buy-in
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 1000
    When I handle a ReserveFunds command with amount 500 for table "table-1"
    Then the result is a FundsReserved event
    And the player event has amount 500
    And the player event has new_available_balance 500

  Scenario: Cannot reserve more than available
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 500
    When I handle a ReserveFunds command with amount 600 for table "table-1"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "insufficient"

  Scenario: Cannot reserve for same table twice
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 1000
    And a FundsReserved event with amount 500 for table "table-1"
    When I handle a ReserveFunds command with amount 200 for table "table-1"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already reserved for this table"

  # --- ReleaseFunds scenarios ---

  Scenario: Release reserved funds back to bankroll
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 1000
    And a FundsReserved event with amount 500 for table "table-1"
    When I handle a ReleaseFunds command for table "table-1"
    Then the result is a FundsReleased event
    And the player event has amount 500
    And the player event has new_available_balance 1000

  Scenario: Cannot release non-existent reservation
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 1000
    When I handle a ReleaseFunds command for table "table-1"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "No funds reserved"

  # --- State reconstruction scenarios ---

  Scenario: Rebuild state with deposits and reservations
    Given a PlayerRegistered event for "Alice"
    And a FundsDeposited event with amount 1000
    And a FundsReserved event with amount 400 for table "table-1"
    When I rebuild the player state
    Then the player state has bankroll 1000
    And the player state has reserved_funds 400
    And the player state has available_balance 600
