Feature: Poker Game Flow
  End-to-end acceptance tests for the poker example application. These tests
  exercise the full angzarr stack: aggregates, sagas, process managers, and
  projectors working together across player, table, and hand domains.

  Why acceptance tests matter:
  - Unit tests verify individual components; acceptance tests verify integration
  - These tests run against the deployed system (standalone or Kubernetes)
  - They validate that cross-domain sagas actually propagate events/commands
  - They catch configuration and wiring issues that unit tests miss

  Patterns exercised by these acceptance tests:
  - Multi-aggregate workflows: Player→Table→Hand coordination via sagas/PMs
  - Event-driven choreography: No central orchestrator - events trigger sagas
  - Compensation flows: Failed JoinTable triggers FundsReleased
  - Async event propagation: "within N seconds" assertions handle saga latency

  Why poker provides effective acceptance tests:
  - Clear business outcomes: "Bob wins $100" is easy to verify
  - Visible cross-domain effects: player balance changes when hand completes
  - Deterministic replay: seeded decks make showdown outcomes predictable
  - Rich edge cases: all-in, side pots, elimination - real complexity

  What these tests demonstrate:
  - Player lifecycle: registration, deposits, fund reservation
  - Table lifecycle: creation, player seating, hand orchestration
  - Hand lifecycle: dealing, betting, community cards, showdown
  - Saga coordination: HandStarted→CardsDealt, PotAwarded→FundsDeposited

  Background:
    Given the poker system is running in standalone mode

  # ===========================================================================
  # Player Registration and Bankroll
  # ===========================================================================
  # These scenarios verify the player aggregate handles registration and
  # fund management correctly. Players must have funds to join tables.

  @e2e @player
  Scenario: Register player and deposit funds
    When I register player "Alice" with email "alice@example.com"
    And I deposit 1000 chips to player "Alice"
    Then player "Alice" has bankroll 1000
    And player "Alice" has available balance 1000

  # ===========================================================================
  # Table Setup and Player Joining
  # ===========================================================================
  # Tables coordinate player seating and hand orchestration. When players join,
  # their funds are reserved (via saga to player domain). These tests verify
  # the cross-domain fund reservation flow works correctly.

  @e2e @table
  Scenario: Create table and seat players
    Given registered players with bankroll:
      | name  | bankroll |
      | Alice | 1000     |
      | Bob   | 1000     |
    When I create a Texas Hold'em table "Main" with blinds 5/10
    And player "Alice" joins table "Main" at seat 0 with buy-in 500
    And player "Bob" joins table "Main" at seat 1 with buy-in 500
    Then table "Main" has 2 seated players
    And player "Alice" has reserved funds 500
    And player "Alice" has available balance 500

  @e2e @table
  Scenario: Player leaves table and recovers funds
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When player "Alice" leaves table "Main"
    Then player "Alice" has bankroll 1000
    And player "Alice" has reserved funds 0
    And table "Main" has 1 seated player

  # ===========================================================================
  # Hand Lifecycle - Basic Flow
  # ===========================================================================
  # Hand lifecycle involves multiple aggregates and sagas. StartHand on table
  # triggers HandStarted, which triggers the table-hand saga to issue DealCards.
  # These tests verify the saga coordination completes within expected time.

  @e2e @hand
  Scenario: Complete heads-up hand with fold
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When a hand starts at table "Main"
    Then within 2 seconds:
      | domain | event_type  |
      | table  | HandStarted |
      | hand   | CardsDealt  |
    When "Alice" posts small blind 5
    And "Bob" posts big blind 10
    And "Alice" folds
    Then "Bob" wins the pot of 15
    And within 2 seconds:
      | domain | event_type   |
      | hand   | HandComplete |
      | table  | HandEnded    |
    And "Alice" stack is 495
    And "Bob" stack is 505

  @e2e @hand
  Scenario: Complete hand through showdown
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And deterministic deck seed "showdown-test"
    When a hand starts at table "Main"
    And blinds are posted (5/10)
    And "Alice" calls 10
    And "Bob" checks
    Then the flop is dealt
    When "Bob" checks
    And "Alice" checks
    Then the turn is dealt
    When "Bob" checks
    And "Alice" checks
    Then the river is dealt
    When "Bob" checks
    And "Alice" checks
    Then showdown begins
    And the winner is determined by hand ranking
    And the hand completes

  # ===========================================================================
  # Betting Actions
  # ===========================================================================
  # Betting tests verify the hand aggregate correctly validates and processes
  # player actions. The process manager tracks action order and pot totals.

  @e2e @betting
  Scenario: Raise and re-raise sequence
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When a hand starts and blinds are posted (5/10)
    And "Alice" raises to 30
    And "Bob" re-raises to 90
    And "Alice" calls 60
    Then the pot is 180
    And the flop is dealt

  @e2e @betting
  Scenario: All-in and call
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 100   |
      | Bob   | 1    | 500   |
    When a hand starts and blinds are posted (5/10)
    And "Alice" goes all-in for 100
    And "Bob" calls 100
    Then the pot is 200
    And showdown is triggered immediately

  # ===========================================================================
  # Multi-Player Scenarios
  # ===========================================================================
  # Multi-player scenarios test more complex pot calculations including
  # side pots when players go all-in for different amounts.

  @e2e @multiplayer
  Scenario: Three player hand with one fold
    Given a table "Main" with seated players:
      | name   | seat | stack |
      | Alice  | 0    | 500   |
      | Bob    | 1    | 500   |
      | Carol  | 2    | 500   |
    When a hand starts with dealer at seat 0
    Then "Bob" is small blind and "Carol" is big blind
    When blinds are posted (5/10)
    And "Alice" calls 10
    And "Bob" folds
    And "Carol" checks
    Then active player count is 2
    And the pot is 25

  @e2e @multiplayer
  Scenario: Side pot creation with all-in
    Given a table "Main" with seated players:
      | name   | seat | stack |
      | Alice  | 0    | 50    |
      | Bob    | 1    | 500   |
      | Carol  | 2    | 500   |
    When a hand starts and blinds are posted (5/10)
    And "Alice" goes all-in for 50
    And "Bob" calls 50
    And "Carol" raises to 150
    And "Bob" calls 100
    Then there is a main pot of 150 with 3 players eligible
    And there is a side pot of 200 with 2 players eligible

  # ===========================================================================
  # Game Variants
  # ===========================================================================
  # Different poker variants have different rules (hole cards, community cards,
  # draw phases). These tests verify variant-specific logic is correct.

  @e2e @variant
  Scenario: Five Card Draw with discard
    Given a Five Card Draw table "Draw" with blinds 5/10
    And seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When a hand starts at table "Draw"
    And blinds are posted (5/10)
    And "Alice" calls 10
    And "Bob" checks
    Then the draw phase begins
    When "Alice" discards 2 cards at indices [0, 1]
    And "Bob" stands pat
    Then "Alice" has 5 hole cards
    And the second betting round begins

  @e2e @variant
  Scenario: Omaha deals 4 hole cards
    Given an Omaha table "Omaha" with blinds 10/20
    And seated players:
      | name  | seat | stack |
      | Alice | 0    | 1000  |
      | Bob   | 1    | 1000  |
    When a hand starts at table "Omaha"
    Then each player has 4 hole cards
    And the remaining deck has 44 cards

  # ===========================================================================
  # Tournament/Session Scenarios
  # ===========================================================================
  # Long-running sessions involve multiple hands with stack changes. Player
  # elimination occurs when stacks reach zero. These test session continuity.

  @e2e @tournament
  Scenario: Player elimination
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 15    |
    When a hand starts and blinds are posted (5/10)
    And "Alice" raises to 30
    And "Bob" goes all-in for 15
    And "Alice" calls 0
    And showdown occurs with "Alice" winning
    Then "Bob" has stack 0
    And "Bob" is eliminated from table "Main"
    And table "Main" has 1 seated player

  @e2e @tournament
  Scenario: Multiple hands with stack changes
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When hand 1 completes with "Alice" winning 50
    Then "Alice" has stack 550
    And "Bob" has stack 450
    When hand 2 completes with "Bob" winning 100
    Then "Alice" has stack 450
    And "Bob" has stack 550
    And table "Main" has hand_count 2

  # ===========================================================================
  # Saga Coordination
  # ===========================================================================
  # These tests specifically verify saga-mediated cross-domain workflows.
  # The "within N seconds" assertions allow for async saga processing.

  @e2e @saga
  Scenario: HandStarted triggers DealCards via saga
    Given a table "Main" with 2 seated players
    When I send a StartHand command to table "Main"
    Then within 3 seconds:
      | domain | event_type  |
      | table  | HandStarted |
      | hand   | CardsDealt  |
    And the hand has the same hand_number as the table event

  @e2e @saga
  Scenario: HandComplete triggers EndHand via saga
    Given a table "Main" with an active hand
    When the hand completes with winner "Alice"
    Then within 3 seconds:
      | domain | event_type   |
      | hand   | HandComplete |
      | table  | HandEnded    |
    And the table updates player stacks

  # ===========================================================================
  # Error Handling
  # ===========================================================================
  # Invalid commands should be rejected with clear error messages. These tests
  # verify business rule validation works end-to-end.

  @e2e @error
  Scenario: Reject action from wrong player
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And a hand is dealt with "Alice" to act
    When "Bob" attempts to act
    Then the command fails with "not your turn"

  @e2e @error
  Scenario: Reject invalid bet amount
    Given a table "Main" with an active hand
    And current bet is 10 and min raise is 10
    When player attempts to raise to 15
    Then the command fails with "minimum raise"

  # ===========================================================================
  # Split Pots and Tie Scenarios
  # ===========================================================================
  # When multiple players have identical hand strength, the pot is divided
  # equally among them. This is common with paired boards or shared straights.

  @e2e @showdown
  Scenario: Split pot when both players have identical hands
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And deterministic deck where both players make the same flush
    When a hand starts and blinds are posted (5/10)
    And both players check to showdown
    Then the pot of 20 is split evenly
    And "Alice" wins 10
    And "Bob" wins 10

  @e2e @showdown
  Scenario: Chopped board - community cards make the best hand
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And deterministic deck with community cards making a royal flush
    When a hand completes through showdown
    Then both players play the board
    And the pot is split evenly

  # ===========================================================================
  # Kicker Determination
  # ===========================================================================
  # When players have the same hand rank (e.g., pair of aces), the kicker
  # cards determine the winner. This is fundamental to poker hand comparison.

  @e2e @showdown
  Scenario: Higher kicker wins with same pair
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And deterministic deck where:
      | player | hole_cards | community         |
      | Alice  | Ah Kc      | As 9d 7h 4c 2s    |
      | Bob    | Ad Qc      | As 9d 7h 4c 2s    |
    When a hand completes through showdown
    Then both players have a pair of aces
    And "Alice" wins with king kicker over queen

  # ===========================================================================
  # Check-Raise
  # ===========================================================================
  # A check-raise is a deceptive play: check to induce a bet, then raise.
  # This tests that checking doesn't forfeit the right to raise later.

  @e2e @betting
  Scenario: Check-raise on the flop
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When a hand starts and blinds are posted (5/10)
    And preflop betting completes with calls
    And the flop is dealt
    When "Alice" checks
    And "Bob" bets 20
    And "Alice" raises to 60
    Then the pot is 100
    And "Bob" must act

  # ===========================================================================
  # Heads-Up Specific Rules
  # ===========================================================================
  # In heads-up play (2 players), the button posts the small blind and acts
  # first preflop. This differs from multi-way play where the small blind
  # is to the left of the button.

  @e2e @headsup
  Scenario: Heads-up blind posting - button is small blind
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When a hand starts with dealer at seat 0
    Then "Alice" posts the small blind of 5
    And "Bob" posts the big blind of 10
    And "Alice" acts first preflop

  @e2e @headsup
  Scenario: Heads-up - big blind wins when button folds preflop
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When a hand starts and blinds are posted (5/10)
    And "Alice" folds
    Then "Bob" wins the pot of 15 uncontested
    And no showdown occurs

  # ===========================================================================
  # Walking the Blinds
  # ===========================================================================
  # When all players fold to the big blind preflop, the big blind "walks"
  # and wins the pot (including the small blind) without playing a hand.

  @e2e @multiplayer
  Scenario: Big blind walks - everyone folds preflop
    Given a table "Main" with seated players:
      | name   | seat | stack |
      | Alice  | 0    | 500   |
      | Bob    | 1    | 500   |
      | Carol  | 2    | 500   |
    When a hand starts with dealer at seat 0
    And blinds are posted (5/10)
    And "Alice" folds
    And "Bob" folds
    Then "Carol" wins the pot of 15 uncontested
    And the hand ends without showdown

  # ===========================================================================
  # Multiple Side Pots
  # ===========================================================================
  # When multiple players go all-in for different amounts, side pots form.
  # Each player can only win from pots they contributed to.

  @e2e @sidepots
  Scenario: Three side pots with four players
    Given a table "Main" with seated players:
      | name   | seat | stack |
      | Alice  | 0    | 50    |
      | Bob    | 1    | 150   |
      | Carol  | 2    | 300   |
      | Dave   | 3    | 500   |
    When a hand starts and blinds are posted (5/10)
    And "Alice" goes all-in for 50
    And "Bob" goes all-in for 150
    And "Carol" goes all-in for 300
    And "Dave" calls 300
    Then there is a main pot of 200 with 4 players eligible
    And there is a side pot of 300 with 3 players eligible
    And there is a side pot of 300 with 2 players eligible

  @e2e @sidepots
  Scenario: Side pot awarded to second-best hand when main pot winner is all-in
    Given a table "Main" with seated players:
      | name   | seat | stack |
      | Alice  | 0    | 50    |
      | Bob    | 1    | 500   |
      | Carol  | 2    | 500   |
    And deterministic deck where Alice has best hand, Bob has second best
    When a hand starts and blinds are posted (5/10)
    And "Alice" goes all-in for 50
    And "Bob" raises to 200
    And "Carol" calls 200
    And showdown occurs
    Then "Alice" wins main pot of 150
    And "Bob" wins side pot of 300

  # ===========================================================================
  # All-In Below Minimum Raise
  # ===========================================================================
  # When a player goes all-in for less than the minimum raise, action
  # reopens only if the all-in is at least half the minimum raise. Otherwise,
  # players who already acted cannot re-raise.

  @e2e @betting
  Scenario: All-in below min-raise does not reopen action
    Given a table "Main" with seated players:
      | name   | seat | stack |
      | Alice  | 0    | 500   |
      | Bob    | 1    | 25    |
      | Carol  | 2    | 500   |
    When a hand starts and blinds are posted (5/10)
    And "Alice" raises to 30
    And "Bob" goes all-in for 25
    Then "Carol" may call 30 or raise to at least 50
    But "Alice" may only call 0 if "Carol" just calls

  @e2e @betting
  Scenario: All-in above half min-raise reopens action
    Given a table "Main" with seated players:
      | name   | seat | stack |
      | Alice  | 0    | 500   |
      | Bob    | 1    | 45    |
      | Carol  | 2    | 500   |
    When a hand starts and blinds are posted (5/10)
    And "Alice" raises to 30
    And "Bob" goes all-in for 45
    Then "Carol" may call 45 or raise to at least 60
    And "Alice" may re-raise if "Carol" raises

  # ===========================================================================
  # Rebuy - Adding Chips Between Hands
  # ===========================================================================
  # Players can add chips to their stack between hands. This requires
  # coordination between player bankroll (where funds come from) and
  # table stack (where chips go).

  @e2e @rebuy
  Scenario: Player adds chips between hands
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 200   |
      | Bob   | 1    | 500   |
    And player "Alice" has bankroll 1000 with 200 reserved
    When hand 1 completes
    And "Alice" adds 300 chips to her stack
    Then "Alice" has stack 500
    And player "Alice" has reserved funds 500
    And player "Alice" has available balance 500

  @e2e @rebuy
  Scenario: Cannot add chips during active hand
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 200   |
      | Bob   | 1    | 500   |
    And a hand is in progress
    When "Alice" attempts to add chips
    Then the request fails with "cannot add chips during hand"

  @e2e @rebuy
  Scenario: Cannot add chips beyond available bankroll
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 200   |
      | Bob   | 1    | 500   |
    And player "Alice" has bankroll 300 with 200 reserved
    When "Alice" attempts to add 200 chips
    Then the request fails with "insufficient funds"
