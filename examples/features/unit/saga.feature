Feature: Saga logic
  Sagas translate events from one domain into commands for another. They're
  stateless domain bridges that enable loose coupling between aggregates.
  Each saga handles one direction: table→hand or hand→player.

  Why sagas exist:
  - Aggregates shouldn't know about each other directly
  - Domain translation logic has a clear home
  - Saga failures can be compensated independently
  - Adding new sagas extends functionality without changing aggregates

  Patterns enabled by sagas:
  - Domain decoupling: Table doesn't import Hand types; saga translates. Same
    pattern applies to order→fulfillment, payment→ledger, user→notification.
  - Event-driven choreography: Events trigger sagas; sagas emit commands. No
    central orchestrator needed. Same pattern applies to microservice integration.
  - Fan-out reactions: One event can trigger multiple sagas targeting different
    domains. PotAwarded triggers both player balance updates AND table state updates.

  Why poker exercises saga patterns well:
  - Clear domain boundaries: player (money), table (seating), hand (gameplay)
  - Obvious translations: HandStarted→DealCards, PotAwarded→DepositFunds
  - Multi-target fan-out: HandComplete triggers hand-player-saga AND hand-table-saga
  - Compensation scenarios: JoinTable rejection requires FundsReleased via saga

  The poker example sagas:
  - table-hand-saga: table events → hand commands (HandStarted → DealCards)
  - hand-player-saga: hand events → player commands (PotAwarded → DepositFunds)
  - hand-table-saga: hand events → table commands (HandComplete → EndHand)

  # ==========================================================================
  # TableSyncSaga - Table to Hand Bridge
  # ==========================================================================
  # When a table starts a hand, the saga translates this into commands for
  # the hand aggregate. When a hand completes, it signals the table to end.

  Scenario: Table sync saga routes HandStarted to DealCards
    Given a TableSyncSaga
    And a HandStarted event from table domain with:
      | hand_root   | hand_number | game_variant   | dealer_position |
      | hand-1      | 1           | TEXAS_HOLDEM   | 0               |
    And active players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
      | player-2    | 1        | 500   |
    When the saga handles the event
    Then the saga emits a DealCards command to hand domain
    And the command has game_variant TEXAS_HOLDEM
    And the command has 2 players
    And the command has hand_number 1

  Scenario: Table sync saga routes HandComplete to EndHand
    Given a TableSyncSaga
    And a HandComplete event from hand domain with:
      | table_root | pot_total |
      | table-1    | 100       |
    And winners:
      | player_root | amount |
      | player-1    | 100    |
    When the saga handles the event
    Then the saga emits an EndHand command to table domain
    And the command has 1 result
    And the result has winner "player-1" with amount 100

  # ==========================================================================
  # HandResultsSaga - Hand to Player Bridge
  # ==========================================================================
  # When a hand ends or pots are awarded, players' bankrolls need updating.
  # This saga emits DepositFunds/ReleaseFunds commands to the player domain.

  Scenario: Hand results saga routes HandEnded to ReleaseFunds
    Given a HandResultsSaga
    And a HandEnded event from table domain with:
      | hand_root |
      | hand-1    |
    And stack_changes:
      | player_root | change |
      | player-1    | 50     |
      | player-2    | -50    |
    When the saga handles the event
    Then the saga emits 2 ReleaseFunds commands to player domain

  Scenario: Hand results saga routes PotAwarded to DepositFunds
    Given a HandResultsSaga
    And a PotAwarded event from hand domain with:
      | pot_total |
      | 100       |
    And winners:
      | player_root | amount |
      | player-1    | 60     |
      | player-2    | 40     |
    When the saga handles the event
    Then the saga emits 2 DepositFunds commands to player domain
    And the first command has amount 60 for "player-1"
    And the second command has amount 40 for "player-2"

  # ==========================================================================
  # SagaRouter Infrastructure
  # ==========================================================================
  # The SagaRouter dispatches events to matching saga handlers. Multiple
  # sagas can be registered; only those matching the event type are invoked.

  Scenario: Saga router dispatches to matching sagas only
    Given a SagaRouter with TableSyncSaga and HandResultsSaga
    And a HandStarted event
    When the router routes the event
    Then only TableSyncSaga handles the event

  Scenario: Saga router handles multiple events in event book
    Given a SagaRouter with TableSyncSaga
    And an event book with:
      | event_type   |
      | HandStarted  |
      | HandStarted  |
    When the router routes the events
    Then the saga emits 2 DealCards commands

  Scenario: Saga router continues after saga failure
    Given a SagaRouter with a failing saga and TableSyncSaga
    And a HandStarted event
    When the router routes the event
    Then TableSyncSaga still emits its command
    And no exception is raised
