@wip
Feature: Projector logic
  The OutputProjector transforms domain events into human-readable text.
  It's a read-model builder that enables observability without coupling
  the game logic to any specific output format.

  Why projectors matter:
  - Aggregates focus on business rules, not presentation
  - The same events can drive multiple projectors (text, JSON, WebSocket)
  - Projectors can be deployed/updated independently of aggregates

  Patterns enabled by projectors:
  - Read model denormalization: Combine data from multiple event types into
    query-optimized views. Same pattern applies to search indexes, dashboards.
  - Event stream formatting: Transform events for external systems (logs, APIs,
    WebSockets). Same pattern applies to audit logs, analytics pipelines.
  - Stateful context building: Track cross-event state (player names) for
    enriched output. Same pattern applies to session tracking, entity resolution.
  - Multi-domain subscription: Single projector consumes player, table, AND hand
    events. Same pattern applies to unified dashboards, cross-cutting analytics.

  Why poker exercises projector patterns well:
  - Multiple event types: PlayerRegistered, TableCreated, CardsDealt, ActionTaken,
    CommunityCardsDealt, PotAwarded - each needs different formatting
  - Cross-event context: Player names from registration used when formatting
    ActionTaken events - requires stateful tracking
  - High-frequency updates: 20+ events per hand means projector sees rapid flow
  - Domain variety: Events from player, table, and hand domains all flow through

  What this projector demonstrates:
  - Stateful context (player names) built from registration events
  - Event-specific formatting (cards, blinds, actions)
  - Graceful handling of unknown events

  # ==========================================================================
  # Player Event Rendering
  # ==========================================================================
  # Player events establish context (names, balances) used throughout
  # the game display. Projectors often need to track cross-event state.

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

  # ==========================================================================
  # Table Event Rendering
  # ==========================================================================
  # Table events describe the game structure: table creation, player seating,
  # and hand lifecycle. These set up the context for hand-level events.

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

  # ==========================================================================
  # Hand Event Rendering
  # ==========================================================================
  # Hand events are the most frequent and detailed. Each betting action,
  # community card, and showdown reveal needs clear, consistent formatting.

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

  # ==========================================================================
  # Card Formatting
  # ==========================================================================
  # Cards are represented as protobuf messages (suit + rank integers).
  # The projector converts these to standard notation: "As" = Ace of Spades.

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

  # ==========================================================================
  # Player Name Resolution
  # ==========================================================================
  # Events reference players by root ID (e.g., "player-abc123"). The projector
  # maintains a name cache built from PlayerRegistered events. This separation
  # keeps event payloads small while enabling friendly display names.

  Scenario: Use registered player names
    Given an OutputProjector
    And player "player-abc123" is registered as "Alice"
    When an event references "player-abc123"
    Then the output uses "Alice"

  Scenario: Fallback to truncated player ID
    Given an OutputProjector
    When an event references unknown "player-xyz789"
    Then the output uses "Player_xyz789" prefix

  # ==========================================================================
  # Timestamp Display
  # ==========================================================================
  # Timestamps are useful for debugging but can clutter normal output.
  # The projector supports toggling timestamp display via configuration.

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

  # ==========================================================================
  # Event Book Processing
  # ==========================================================================
  # Commands often produce multiple events. The projector must handle batches
  # correctly and gracefully degrade when encountering unknown event types.

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
