Feature: Table aggregate logic
  The Table aggregate manages a poker table session: configuration, player
  seating, and hand lifecycle. It's the orchestration layer between players
  (who have money) and hands (where money changes ownership).

  Why this aggregate exists:
  - Tables have configuration (blinds, limits) that hands inherit
  - Player seating is table-scoped, not hand-scoped
  - Dealer button and hand numbering track across multiple hands
  - Players join/leave tables, not individual hands

  What breaks if this is wrong:
  - Players could be double-seated at the same table
  - Hands could start with insufficient players
  - Dealer button wouldn't advance correctly

  Patterns enabled by this aggregate:
  - Cross-aggregate coordination: Table emits HandStarted, triggering saga to
    create Hand aggregate. Same pattern applies to order→fulfillment, auction→bid.
  - Slot/capacity management: Seats are exclusive resources with validation.
    Same pattern applies to parking spots, meeting room bookings, flight seats.
  - Child aggregate lifecycle: Table spawns hands, tracks their completion,
    updates state. Same pattern applies to project→tasks, tournament→matches.

  Why poker exercises these patterns well:
  - Seat occupancy is binary and obvious: seat 3 either has a player or doesn't
  - Hand lifecycle has clear start/end: HandStarted→HandEnded, easy to verify
  - Configuration inheritance is explicit: blinds flow from table to hand
  - Concurrent state is visible: 2 players at seats 0,3 while seats 1,2 empty

  # ==========================================================================
  # Table Creation
  # ==========================================================================
  # Tables are created with game configuration. Once created, the table
  # exists until closed (future feature). Duplicate creation is rejected.

  Scenario: Create a Texas Hold'em table
    Given no prior events for the table aggregate
    When I handle a CreateTable command with name "Main Table" and variant "TEXAS_HOLDEM":
      | small_blind | big_blind | min_buy_in | max_buy_in | max_players |
      | 5           | 10        | 200        | 1000       | 9           |
    Then the result is a TableCreated event
    And the table event has table_name "Main Table"
    And the table event has game_variant "TEXAS_HOLDEM"
    And the table event has small_blind 5
    And the table event has big_blind 10

  Scenario: Create a Five Card Draw table
    Given no prior events for the table aggregate
    When I handle a CreateTable command with name "Draw Table" and variant "FIVE_CARD_DRAW":
      | small_blind | big_blind | min_buy_in | max_buy_in | max_players |
      | 10          | 20        | 400        | 2000       | 6           |
    Then the result is a TableCreated event
    And the table event has game_variant "FIVE_CARD_DRAW"

  Scenario: Cannot create table twice
    Given a TableCreated event for "Main Table"
    When I handle a CreateTable command with name "Another Table" and variant "TEXAS_HOLDEM":
      | small_blind | big_blind | min_buy_in | max_buy_in | max_players |
      | 5           | 10        | 200        | 1000       | 9           |
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already exists"

  # ==========================================================================
  # Player Seating
  # ==========================================================================
  # Players join with a buy-in (chips for play). The table tracks occupied
  # seats. Players can request a specific seat or take any available one.
  # Join failures don't affect the player's bankroll - no funds reserved yet.

  Scenario: Player joins table at preferred seat
    Given a TableCreated event for "Main Table"
    When I handle a JoinTable command for player "player-1" at seat 3 with buy-in 500
    Then the result is a PlayerJoined event
    And the table event has seat_position 3
    And the table event has buy_in_amount 500

  Scenario: Player joins table at any seat
    Given a TableCreated event for "Main Table"
    When I handle a JoinTable command for player "player-1" at seat -1 with buy-in 500
    Then the result is a PlayerJoined event
    And the table event has seat_position 0

  Scenario: Cannot join occupied seat
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 3
    When I handle a JoinTable command for player "player-2" at seat 3 with buy-in 500
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "Seat is occupied"

  Scenario: Cannot join table twice
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 3
    When I handle a JoinTable command for player "player-1" at seat 5 with buy-in 500
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already seated"

  Scenario: Cannot join with insufficient buy-in
    Given a TableCreated event for "Main Table" with min_buy_in 200
    When I handle a JoinTable command for player "player-1" at seat 0 with buy-in 100
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "Buy-in must be at least"

  Scenario: Cannot join full table
    Given a TableCreated event for "Main Table" with max_players 2
    And a PlayerJoined event for player "player-1" at seat 0
    And a PlayerJoined event for player "player-2" at seat 1
    When I handle a JoinTable command for player "player-3" at seat -1 with buy-in 500
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "Table is full"

  # ==========================================================================
  # Player Departure
  # ==========================================================================
  # Players leave with their remaining stack (may differ from buy-in).
  # Departure during an active hand is forbidden - the player must wait
  # for the hand to complete. This prevents mid-hand bailouts.

  Scenario: Player leaves table
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 3 with stack 500
    When I handle a LeaveTable command for player "player-1"
    Then the result is a PlayerLeft event
    And the table event has chips_cashed_out 500

  Scenario: Cannot leave during hand
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0
    And a PlayerJoined event for player "player-2" at seat 1
    And a HandStarted event for hand 1
    When I handle a LeaveTable command for player "player-1"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "during a hand"

  Scenario: Cannot leave table not joined
    Given a TableCreated event for "Main Table"
    When I handle a LeaveTable command for player "player-1"
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "not seated"

  # ==========================================================================
  # Hand Lifecycle - Start
  # ==========================================================================
  # Starting a hand captures the current player stacks and advances the
  # dealer button. The HandStarted event triggers the hand-table-saga to
  # deal cards in the hand domain.

  Scenario: Start a new hand
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0 with stack 500
    And a PlayerJoined event for player "player-2" at seat 1 with stack 500
    When I handle a StartHand command
    Then the result is a HandStarted event
    And the table event has hand_number 1
    And the table event has 2 active_players

  Scenario: Dealer button advances each hand
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0
    And a PlayerJoined event for player "player-2" at seat 1
    And a HandStarted event for hand 1 with dealer at seat 0
    And a HandEnded event for hand 1
    When I handle a StartHand command
    Then the result is a HandStarted event
    And the table event has hand_number 2
    And the table event has dealer_position 1

  Scenario: Cannot start hand with fewer than 2 players
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0
    When I handle a StartHand command
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "Not enough players"

  Scenario: Cannot start hand while one is in progress
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0
    And a PlayerJoined event for player "player-2" at seat 1
    And a HandStarted event for hand 1
    When I handle a StartHand command
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already in progress"

  # ==========================================================================
  # Hand Lifecycle - End
  # ==========================================================================
  # Ending a hand applies stack changes (wins/losses) to seated players.
  # The HandEnded event triggers the hand-player-saga to update player
  # bankrolls in the player domain.

  Scenario: End hand and update stacks
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0 with stack 500
    And a PlayerJoined event for player "player-2" at seat 1 with stack 500
    And a HandStarted event for hand 1
    When I handle an EndHand command with winner "player-1" winning 50
    Then the result is a HandEnded event
    And player "player-1" stack change is 50

  Scenario: Cannot end hand not in progress
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0
    And a PlayerJoined event for player "player-2" at seat 1
    When I handle an EndHand command with winner "player-1" winning 50
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "No hand in progress"

  # ==========================================================================
  # State Reconstruction
  # ==========================================================================
  # Table state is rebuilt by replaying events. This verifies that joining,
  # leaving, and hand events correctly update seated players and table status.

  Scenario: Rebuild state with multiple players
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0 with stack 500
    And a PlayerJoined event for player "player-2" at seat 3 with stack 800
    When I rebuild the table state
    Then the table state has 2 players
    And the table state has seat 0 occupied by "player-1"
    And the table state has seat 3 occupied by "player-2"
    And the table state has status "waiting"

  Scenario: Rebuild state during hand
    Given a TableCreated event for "Main Table"
    And a PlayerJoined event for player "player-1" at seat 0
    And a PlayerJoined event for player "player-2" at seat 1
    And a HandStarted event for hand 1
    When I rebuild the table state
    Then the table state has status "in_hand"
    And the table state has hand_count 1
