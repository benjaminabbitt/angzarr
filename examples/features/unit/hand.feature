Feature: Hand aggregate logic
  The Hand aggregate manages a single poker hand: dealing, betting rounds,
  community cards, and showdown. Each hand is an isolated consistency
  boundary with its own event stream.

  Why this aggregate exists:
  - Hands have complex, well-defined state machines (phases, betting rounds)
  - Hand-level events (ActionTaken, CardsDealt) are high-frequency
  - Hand logic is game-variant-specific (Hold'em vs Omaha vs Draw)
  - Separating from table enables parallel hand processing (multi-table)

  What breaks if this is wrong:
  - Players could act out of turn
  - Betting amounts could violate minimum raise rules
  - Community cards could be dealt in wrong phases
  - Showdown could award pots incorrectly

  Patterns enabled by this aggregate:
  - State machine enforcement: DEALING→BLINDS→BETTING→FLOP→... Each phase has
    valid transitions; invalid actions rejected. Same pattern applies to
    order fulfillment, insurance claims, approval workflows.
  - Turn-based action tracking: Only one player can act at a time. Same pattern
    applies to board games, auction rounds, approval chains.
  - High-frequency event streams: 20+ events per hand exercises snapshot
    optimization. Same pattern applies to IoT sensors, trading systems.
  - Variant polymorphism: Same aggregate handles Hold'em/Omaha/Draw with
    different rules. Same pattern applies to payment methods, shipping carriers.

  Why poker exercises these patterns well:
  - State transitions are unambiguous: can't deal turn before flop
  - Turn order is strictly enforced: only position 2 can act when action_on=2
  - Event frequency is high: BlindPosted, ActionTaken×N, CommunityCardsDealt×3
  - Rules vary by variant: 2 hole cards (Hold'em) vs 4 (Omaha) vs 5 (Draw)

  # ==========================================================================
  # Card Dealing
  # ==========================================================================
  # Dealing initializes the hand with hole cards for each player. The number
  # of hole cards depends on game variant (2 for Hold'em, 4 for Omaha, 5 for Draw).
  # Deterministic seeding enables reproducible tests.

  Scenario: Deal Texas Hold'em hand to 2 players
    Given no prior events for the hand aggregate
    When I handle a DealCards command for TEXAS_HOLDEM with players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
      | player-2    | 1        | 500   |
    Then the result is a CardsDealt event
    And each player has 2 hole cards
    And the remaining deck has 48 cards

  Scenario: Deal Omaha hand to 3 players
    Given no prior events for the hand aggregate
    When I handle a DealCards command for OMAHA with players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
      | player-2    | 1        | 500   |
      | player-3    | 2        | 500   |
    Then the result is a CardsDealt event
    And each player has 4 hole cards
    And the remaining deck has 40 cards

  Scenario: Deal Five Card Draw hand to 4 players
    Given no prior events for the hand aggregate
    When I handle a DealCards command for FIVE_CARD_DRAW with players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
      | player-2    | 1        | 500   |
      | player-3    | 2        | 500   |
      | player-4    | 3        | 500   |
    Then the result is a CardsDealt event
    And each player has 5 hole cards
    And the remaining deck has 32 cards

  Scenario: Deterministic shuffle with seed
    Given no prior events for the hand aggregate
    When I handle a DealCards command with seed "test-seed-123" and players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
      | player-2    | 1        | 500   |
    Then the result is a CardsDealt event
    And player "player-1" has specific hole cards for seed "test-seed-123"

  Scenario: Cannot deal cards twice
    Given a CardsDealt event for hand 1
    When I handle a DealCards command for TEXAS_HOLDEM with players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
      | player-2    | 1        | 500   |
    Then the command fails with status "FAILED_PRECONDITION"
    And the error message contains "already dealt"

  Scenario: Cannot deal with fewer than 2 players
    Given no prior events for the hand aggregate
    When I handle a DealCards command for TEXAS_HOLDEM with players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "at least 2 players"

  # ==========================================================================
  # Blind Posting
  # ==========================================================================
  # Blinds are forced bets that seed the pot and drive action. Small blind
  # is posted first, then big blind. Short-stacked players post all-in blinds.

  Scenario: Post small blind
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    When I handle a PostBlind command for player "player-1" type "small" amount 5
    Then the result is a BlindPosted event
    And the blind event has blind_type "small"
    And the blind event has amount 5
    And the blind event has player_stack 495
    And the blind event has pot_total 5

  Scenario: Post big blind
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And a BlindPosted event for player "player-1" amount 5
    When I handle a PostBlind command for player "player-2" type "big" amount 10
    Then the result is a BlindPosted event
    And the blind event has blind_type "big"
    And the blind event has amount 10
    And the blind event has pot_total 15

  Scenario: Post all-in blind when short-stacked
    Given a CardsDealt event for TEXAS_HOLDEM with players:
      | player_root | position | stack |
      | player-1    | 0        | 3     |
      | player-2    | 1        | 500   |
    When I handle a PostBlind command for player "player-1" type "small" amount 5
    Then the result is a BlindPosted event
    And the blind event has amount 3
    And the blind event has player_stack 0

  # ==========================================================================
  # Player Actions
  # ==========================================================================
  # Actions are the core gameplay: fold, check, call, bet, raise, all-in.
  # Each action has validation rules (can't check when facing a bet, minimum
  # raise amounts). Invalid actions are rejected, not auto-corrected.

  Scenario: Player folds
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15
    When I handle a PlayerAction command for player "player-1" action FOLD
    Then the result is an ActionTaken event
    And the action event has action "FOLD"

  Scenario: Player checks when no bet
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15
    And a BettingRoundComplete event for preflop
    And a CommunityCardsDealt event for FLOP
    When I handle a PlayerAction command for player "player-1" action CHECK
    Then the result is an ActionTaken event
    And the action event has action "CHECK"

  Scenario: Player calls the big blind
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15 and current_bet 10
    When I handle a PlayerAction command for player "player-1" action CALL amount 5
    Then the result is an ActionTaken event
    And the action event has action "CALL"
    And the action event has amount 5
    And the action event has pot_total 20

  Scenario: Player bets
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15
    And a BettingRoundComplete event for preflop
    And a CommunityCardsDealt event for FLOP
    When I handle a PlayerAction command for player "player-1" action BET amount 20
    Then the result is an ActionTaken event
    And the action event has action "BET"
    And the action event has amount 20
    And the action event has amount_to_call 20

  Scenario: Player raises
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15 and current_bet 10
    When I handle a PlayerAction command for player "player-1" action RAISE amount 30
    Then the result is an ActionTaken event
    And the action event has action "RAISE"
    And the action event has amount 30

  Scenario: Player goes all-in
    Given a CardsDealt event for TEXAS_HOLDEM with players:
      | player_root | position | stack |
      | player-1    | 0        | 50    |
      | player-2    | 1        | 500   |
    And blinds posted with pot 15 and current_bet 10
    When I handle a PlayerAction command for player "player-1" action ALL_IN amount 50
    Then the result is an ActionTaken event
    And the action event has action "ALL_IN"
    And the action event has player_stack 0

  Scenario: Cannot check when facing a bet
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15 and current_bet 10
    When I handle a PlayerAction command for player "player-1" action CHECK
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "cannot check"

  Scenario: Cannot bet less than minimum
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15
    And a BettingRoundComplete event for preflop
    And a CommunityCardsDealt event for FLOP
    When I handle a PlayerAction command for player "player-1" action BET amount 5
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "at least"

  # ==========================================================================
  # Community Cards
  # ==========================================================================
  # Community cards are shared by all players. Hold'em/Omaha have flop (3),
  # turn (1), river (1). Draw games have no community cards. Dealing community
  # cards transitions between betting rounds.

  Scenario: Deal the flop
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And blinds posted with pot 15
    And a BettingRoundComplete event for preflop
    When I handle a DealCommunityCards command with count 3
    Then the result is a CommunityCardsDealt event
    And the event has 3 cards dealt
    And the event has phase "FLOP"
    And the remaining deck decreases by 3

  Scenario: Deal the turn
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players
    And the flop has been dealt
    And a BettingRoundComplete event for flop
    When I handle a DealCommunityCards command with count 1
    Then the result is a CommunityCardsDealt event
    And the event has 1 card dealt
    And the event has phase "TURN"
    And all_community_cards has 4 cards

  Scenario: Deal the river
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players
    And the flop and turn have been dealt
    And a BettingRoundComplete event for turn
    When I handle a DealCommunityCards command with count 1
    Then the result is a CommunityCardsDealt event
    And the event has phase "RIVER"
    And all_community_cards has 5 cards

  Scenario: Cannot deal community cards in Five Card Draw
    Given a CardsDealt event for FIVE_CARD_DRAW with 2 players
    And blinds posted with pot 15
    And a BettingRoundComplete event for preflop
    When I handle a DealCommunityCards command with count 3
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "community cards"

  # ==========================================================================
  # Draw Phase (Five Card Draw)
  # ==========================================================================
  # In draw games, players discard and receive new cards. Standing pat means
  # keeping all cards. Draw is only valid in draw game variants.

  Scenario: Player discards and draws cards
    Given a CardsDealt event for FIVE_CARD_DRAW with 2 players
    And blinds posted with pot 15
    And a BettingRoundComplete event for preflop
    When I handle a RequestDraw command for player "player-1" discarding indices [0, 2, 4]
    Then the result is a DrawCompleted event
    And the draw event has cards_discarded 3
    And the draw event has cards_drawn 3
    And player "player-1" has 5 hole cards

  Scenario: Player stands pat (no discard)
    Given a CardsDealt event for FIVE_CARD_DRAW with 2 players
    And blinds posted with pot 15
    And a BettingRoundComplete event for preflop
    When I handle a RequestDraw command for player "player-1" discarding indices []
    Then the result is a DrawCompleted event
    And the draw event has cards_discarded 0
    And the draw event has cards_drawn 0

  Scenario: Cannot draw in Texas Hold'em
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players
    And blinds posted with pot 15
    When I handle a RequestDraw command for player "player-1" discarding indices [0]
    Then the command fails with status "INVALID_ARGUMENT"
    And the error message contains "not supported"

  # ==========================================================================
  # Showdown - Card Reveal
  # ==========================================================================
  # At showdown, remaining players reveal or muck their cards. Revealing
  # triggers hand evaluation; mucking concedes without showing.

  Scenario: Player reveals cards at showdown
    Given a completed betting for TEXAS_HOLDEM with 2 players
    And a ShowdownStarted event for the hand
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the reveal event has cards for player "player-1"
    And the reveal event has a hand ranking

  Scenario: Player mucks cards
    Given a completed betting for TEXAS_HOLDEM with 2 players
    And a ShowdownStarted event for the hand
    When I handle a RevealCards command for player "player-1" with muck true
    Then the result is a CardsMucked event

  # ==========================================================================
  # Pot Award
  # ==========================================================================
  # Pots are awarded after showdown (best hand) or when all but one player
  # folds. Awarding the pot triggers HandComplete and returns control to table.

  Scenario: Award pot to single winner
    Given a completed betting for TEXAS_HOLDEM with 2 players
    And a CardsRevealed event for player "player-1" with ranking FLUSH
    And a CardsMucked event for player "player-2"
    When I handle an AwardPot command with winner "player-1" amount 15
    Then the result is a PotAwarded event
    And the award event has winner "player-1" with amount 15

  Scenario: Award pot generates HandComplete
    Given a completed betting for TEXAS_HOLDEM with 2 players
    When I handle an AwardPot command with winner "player-1" amount 15
    Then a HandComplete event is emitted
    And the hand status is "complete"

  # ==========================================================================
  # Hand Evaluation Logic
  # ==========================================================================
  # Hand ranking (high card through royal flush) determines winners. These
  # scenarios verify the evaluator correctly ranks hands and compares kickers.

  Scenario: Royal flush beats straight flush
    Given a showdown with player hands:
      | player   | hole_cards | community_cards    |
      | player-1 | As Ks      | Qs Js Ts 2c 3d     |
      | player-2 | 9s 8s      | Qs Js Ts 2c 3d     |
    When hands are evaluated
    Then player "player-1" has ranking "ROYAL_FLUSH"
    And player "player-2" has ranking "STRAIGHT_FLUSH"
    And player "player-1" wins

  Scenario: Full house beats flush
    Given a showdown with player hands:
      | player   | hole_cards | community_cards    |
      | player-1 | Ah Ad      | Ac 2d 2h 4h 6h     |
      | player-2 | Kh 7h      | Ac 2d 2h 4h 6h     |
    When hands are evaluated
    Then player "player-1" has ranking "FULL_HOUSE"
    And player "player-2" has ranking "FLUSH"
    And player "player-1" wins

  Scenario: High card comparison with kickers
    Given a showdown with player hands:
      | player   | hole_cards | community_cards    |
      | player-1 | Ah Qc      | Kd Jc 9s 4h 2d     |
      | player-2 | Ah Jd      | Kd Jc 9s 4h 2d     |
    When hands are evaluated
    Then player "player-1" has ranking "HIGH_CARD"
    And player "player-2" has ranking "PAIR"
    And player "player-2" wins

  # ==========================================================================
  # Handler-Level Hand Evaluation
  # ==========================================================================
  # These scenarios verify that RevealCards handlers correctly invoke the
  # evaluator and populate the CardsRevealed event with rankings.

  Scenario: Handler detects straight
    Given a hand at showdown with player "player-1" holding "Th 9c" and community "8d 7s 6h 2c 3d"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "STRAIGHT"

  Scenario: Handler detects wheel straight (A-2-3-4-5)
    Given a hand at showdown with player "player-1" holding "Ah 2c" and community "3d 4s 5h Kc Qd"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "STRAIGHT"

  Scenario: Handler detects straight flush
    Given a hand at showdown with player "player-1" holding "9h 8h" and community "7h 6h 5h 2c 3d"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "STRAIGHT_FLUSH"

  Scenario: Handler detects royal flush
    Given a hand at showdown with player "player-1" holding "As Ks" and community "Qs Js Ts 2c 3d"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "ROYAL_FLUSH"

  Scenario: Handler detects four of a kind
    Given a hand at showdown with player "player-1" holding "Kh Kd" and community "Ks Kc 2h 3d 4s"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "FOUR_OF_A_KIND"

  Scenario: Handler detects full house
    Given a hand at showdown with player "player-1" holding "Ah Ad" and community "Ac 2d 2h 4s 6c"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "FULL_HOUSE"

  Scenario: Handler detects flush
    Given a hand at showdown with player "player-1" holding "Ah 7h" and community "2h 4h 6h Kc Qd"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "FLUSH"

  Scenario: Handler detects three of a kind
    Given a hand at showdown with player "player-1" holding "Jh Jd" and community "Js 2c 4d 6h 8s"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "THREE_OF_A_KIND"

  Scenario: Handler detects two pair
    Given a hand at showdown with player "player-1" holding "Th Td" and community "5s 5c 2h 3d Ks"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "TWO_PAIR"

  Scenario: Handler detects pair
    Given a hand at showdown with player "player-1" holding "Ah Ac" and community "Kd Js 9h 4c 2d"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "PAIR"

  Scenario: Handler detects high card
    Given a hand at showdown with player "player-1" holding "Ah Qc" and community "Kd Js 9h 4c 2d"
    When I handle a RevealCards command for player "player-1" with muck false
    Then the result is a CardsRevealed event
    And the revealed ranking is "HIGH_CARD"

  # ==========================================================================
  # State Reconstruction
  # ==========================================================================
  # Hand state includes phase, community cards, player stacks, and who has
  # folded. These scenarios verify correct state rebuilding from events.

  Scenario: Rebuild state after dealing
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    When I rebuild the hand state
    Then the hand state has phase "PREFLOP"
    And the hand state has status "betting"
    And the hand state has 2 players

  Scenario: Rebuild state with community cards
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players
    And the flop has been dealt
    When I rebuild the hand state
    Then the hand state has 3 community cards
    And the hand state has phase "FLOP"

  Scenario: Rebuild state tracks folded players
    Given a CardsDealt event for TEXAS_HOLDEM with 3 players
    And blinds posted with pot 15
    And player "player-1" folded
    When I rebuild the hand state
    Then player "player-1" has_folded is true
    And active player count is 2
