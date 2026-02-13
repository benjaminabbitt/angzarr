Feature: Saga logic
  Tests saga behavior for cross-domain event coordination.

  # --- TableSyncSaga scenarios ---

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

  # --- HandResultsSaga scenarios ---

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

  # --- SagaRouter scenarios ---

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
