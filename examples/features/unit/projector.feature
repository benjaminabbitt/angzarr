Feature: Projector logic
  Tests output projector for rendering poker events as text.

  # --- Player event rendering ---

  Scenario: Render PlayerRegistered event
    Given an OutputProjector
    And a PlayerRegistered event with display_name "Alice"
    When the projector handles the event
    Then the output contains "Alice registered"

  Scenario: Render FundsDeposited event
    Given an OutputProjector
    And a FundsDeposited event with amount 1000 and new_balance 1000
    When the projector handles the event
    Then the output contains "$1,000"
    And the output contains "balance: $1,000"

  Scenario: Render FundsWithdrawn event
    Given an OutputProjector
    And a FundsWithdrawn event with amount 500 and new_balance 500
    When the projector handles the event
    Then the output contains "Withdrew $500"

  Scenario: Render FundsReserved event
    Given an OutputProjector
    And a FundsReserved event with amount 200
    When the projector handles the event
    Then the output contains "Reserved $200"

  # --- Table event rendering ---

  Scenario: Render TableCreated event
    Given an OutputProjector
    And a TableCreated event with:
      | table_name | game_variant   | small_blind | big_blind | min_buy_in | max_buy_in |
      | Main Table | TEXAS_HOLDEM   | 5           | 10        | 200        | 1000       |
    When the projector handles the event
    Then the output contains "Main Table"
    And the output contains "TEXAS_HOLDEM"
    And the output contains "$5/$10"
    And the output contains "$200 - $1,000"

  Scenario: Render PlayerJoined event
    Given an OutputProjector with player name "Bob"
    And a PlayerJoined event at seat 3 with buy_in 500
    When the projector handles the event
    Then the output contains "Bob joined at seat 3"
    And the output contains "$500"

  Scenario: Render PlayerLeft event
    Given an OutputProjector with player name "Bob"
    And a PlayerLeft event with chips_cashed_out 750
    When the projector handles the event
    Then the output contains "Bob left"
    And the output contains "$750"

  Scenario: Render HandStarted event
    Given an OutputProjector
    And a HandStarted event with:
      | hand_number | dealer_position | small_blind | big_blind |
      | 5           | 2               | 5           | 10        |
    And active players "Alice", "Bob", "Charlie" at seats 0, 1, 2
    When the projector handles the event
    Then the output contains "HAND #5"
    And the output contains "Dealer: Seat 2"
    And the output contains "Alice"
    And the output contains "Bob"
    And the output contains "Charlie"

  Scenario: Render HandEnded event with results
    Given an OutputProjector with player names "Alice" and "Bob"
    And a HandEnded event with winner "Alice" amount 100
    When the projector handles the event
    Then the output contains "Alice wins $100"

  # --- Hand event rendering ---

  Scenario: Render CardsDealt event
    Given an OutputProjector with player name "Alice"
    And a CardsDealt event with player "Alice" holding As Kh
    When the projector handles the event
    Then the output contains "Alice: [As Kh]"

  Scenario: Render BlindPosted event
    Given an OutputProjector with player name "Alice"
    And a BlindPosted event for "Alice" type "small" amount 5
    When the projector handles the event
    Then the output contains "Alice posts SMALL $5"

  Scenario: Render ActionTaken with fold
    Given an OutputProjector with player name "Alice"
    And an ActionTaken event for "Alice" action FOLD
    When the projector handles the event
    Then the output contains "Alice folds"

  Scenario: Render ActionTaken with call
    Given an OutputProjector with player name "Alice"
    And an ActionTaken event for "Alice" action CALL amount 10 pot_total 25
    When the projector handles the event
    Then the output contains "Alice calls $10"
    And the output contains "pot: $25"

  Scenario: Render ActionTaken with raise
    Given an OutputProjector with player name "Alice"
    And an ActionTaken event for "Alice" action RAISE amount 30 pot_total 55
    When the projector handles the event
    Then the output contains "Alice raises to $30"

  Scenario: Render ActionTaken with all-in
    Given an OutputProjector with player name "Alice"
    And an ActionTaken event for "Alice" action ALL_IN amount 500 pot_total 600
    When the projector handles the event
    Then the output contains "Alice all-in $500"

  Scenario: Render CommunityCardsDealt for flop
    Given an OutputProjector
    And a CommunityCardsDealt event for FLOP with cards Ah Kd 7s
    When the projector handles the event
    Then the output contains "Flop: [Ah Kd 7s]"
    And the output contains "Board:"

  Scenario: Render CommunityCardsDealt for turn
    Given an OutputProjector
    And a CommunityCardsDealt event for TURN with card 2c
    When the projector handles the event
    Then the output contains "Turn: [2c]"

  Scenario: Render ShowdownStarted event
    Given an OutputProjector
    And a ShowdownStarted event
    When the projector handles the event
    Then the output contains "SHOWDOWN"

  Scenario: Render CardsRevealed event
    Given an OutputProjector with player name "Alice"
    And a CardsRevealed event for "Alice" with cards As Ad and ranking PAIR
    When the projector handles the event
    Then the output contains "Alice shows [As Ad]"
    And the output contains "Pair"

  Scenario: Render CardsMucked event
    Given an OutputProjector with player name "Alice"
    And a CardsMucked event for "Alice"
    When the projector handles the event
    Then the output contains "Alice mucks"

  Scenario: Render PotAwarded event
    Given an OutputProjector with player name "Alice"
    And a PotAwarded event with winner "Alice" amount 150
    When the projector handles the event
    Then the output contains "Alice wins $150"

  Scenario: Render HandComplete event
    Given an OutputProjector with player names "Alice" and "Bob"
    And a HandComplete event with final stacks:
      | player | stack | has_folded |
      | Alice  | 600   | false      |
      | Bob    | 400   | true       |
    When the projector handles the event
    Then the output contains "Final stacks"
    And the output contains "Alice: $600"
    And the output contains "Bob: $400 (folded)"

  Scenario: Render PlayerTimedOut event
    Given an OutputProjector with player name "Alice"
    And a PlayerTimedOut event for "Alice" with default_action FOLD
    When the projector handles the event
    Then the output contains "Alice timed out"
    And the output contains "auto folds"

  # --- Card formatting scenarios ---

  Scenario: Format card with all suits
    Given an OutputProjector
    When formatting cards:
      | suit     | rank |
      | CLUBS    | 14   |
      | DIAMONDS | 13   |
      | HEARTS   | 12   |
      | SPADES   | 11   |
    Then the output contains "Ac Kd Qh Js"

  Scenario: Format card with all ranks
    Given an OutputProjector
    When formatting cards with rank 2 through 14
    Then ranks 2-9 display as digits
    And rank 10 displays as "T"
    And rank 11 displays as "J"
    And rank 12 displays as "Q"
    And rank 13 displays as "K"
    And rank 14 displays as "A"

  # --- Player name scenarios ---

  Scenario: Use registered player names
    Given an OutputProjector
    And player "player-abc123" is registered as "Alice"
    When an event references "player-abc123"
    Then the output uses "Alice"

  Scenario: Fallback to truncated player ID
    Given an OutputProjector
    When an event references unknown "player-xyz789"
    Then the output uses "Player_xyz789" prefix

  # --- Timestamp scenarios ---

  Scenario: Include timestamps when enabled
    Given an OutputProjector with show_timestamps enabled
    And an event with created_at 14:30:00
    When the projector handles the event
    Then the output starts with "[14:30:00]"

  Scenario: Exclude timestamps when disabled
    Given an OutputProjector with show_timestamps disabled
    And an event with created_at
    When the projector handles the event
    Then the output does not start with "[14:"

  # --- Event book scenarios ---

  Scenario: Handle multiple events in event book
    Given an OutputProjector
    And an event book with PlayerJoined and BlindPosted events
    When the projector handles the event book
    Then both events are rendered in order

  Scenario: Handle unknown event type gracefully
    Given an OutputProjector
    And an event with unknown type_url "type.poker/examples.UnknownEvent"
    When the projector handles the event
    Then the output contains "[Unknown event type:"
