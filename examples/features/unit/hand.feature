Feature: Hand aggregate logic
  Tests hand aggregate behavior for poker hand lifecycle.

  # --- DealCards scenarios ---

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

  # --- PostBlind scenarios ---

  Scenario: Post small blind
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    When I handle a PostBlind command for player "player-1" type "small" amount 5
    Then the result is a BlindPosted event
    And the player event has blind_type "small"
    And the player event has amount 5
    And the player event has player_stack 495
    And the player event has pot_total 5

  Scenario: Post big blind
    Given a CardsDealt event for TEXAS_HOLDEM with 2 players at stacks 500
    And a BlindPosted event for player "player-1" amount 5
    When I handle a PostBlind command for player "player-2" type "big" amount 10
    Then the result is a BlindPosted event
    And the player event has blind_type "big"
    And the player event has amount 10
    And the player event has pot_total 15

  Scenario: Post all-in blind when short-stacked
    Given a CardsDealt event for TEXAS_HOLDEM with players:
      | player_root | position | stack |
      | player-1    | 0        | 3     |
      | player-2    | 1        | 500   |
    When I handle a PostBlind command for player "player-1" type "small" amount 5
    Then the result is a BlindPosted event
    And the player event has amount 3
    And the player event has player_stack 0

  # --- PlayerAction scenarios ---

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

  # --- DealCommunityCards scenarios ---

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

  # --- RequestDraw scenarios (Five Card Draw) ---

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

  # --- RevealCards scenarios ---

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

  # --- AwardPot scenarios ---

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

  # --- Hand evaluation scenarios (test evaluator logic) ---

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

  # --- Handler hand evaluation scenarios (test through RevealCards handler) ---

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

  # --- State reconstruction scenarios ---

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
