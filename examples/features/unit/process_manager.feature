@wip
Feature: Process manager logic
  The HandFlowPM orchestrates a poker hand's state machine: dealing, blind
  posting, betting rounds, community cards, and showdown. Unlike sagas,
  process managers are STATEFUL - they maintain their own event stream to
  track workflow progress.

  Why a process manager (not just sagas):
  - Hand flow requires sequencing: blinds before betting, flop before turn
  - State machine transitions need memory (current phase, who has acted)
  - Timeouts require knowing the current action player
  - Multiple events across domains must be correlated

  Patterns enabled by process managers:
  - Cross-domain correlation: Events from table AND hand domains drive one
    workflow. Same pattern applies to order+payment+shipping coordination.
  - Long-running workflows: A hand may last minutes with many events. PM state
    survives restarts. Same pattern applies to approval chains, onboarding flows.
  - Timeout orchestration: PM schedules timeouts, handles expiration. Same
    pattern applies to payment timeouts, SLA enforcement, auction endings.
  - Compensation coordination: When downstream fails, PM updates workflow state
    before source aggregate compensates. Same pattern for distributed sagas.

  Why poker exercises PM patterns well:
  - Multi-domain events: HandStarted (table) + CardsDealt (hand) + actions (hand)
  - Complex state machine: DEALING→BLINDS→BETTING→FLOP→BETTING→TURN→... with
    clear phase transitions and invalid paths
  - Timeouts are critical: players have N seconds to act or auto-fold/check
  - Action tracking: who has acted, who needs to act, who has folded

  The HandFlowPM:
  - Receives events from table and hand domains
  - Maintains workflow state (phase, betting state, player status)
  - Emits commands to advance the hand (PostBlind, DealCommunityCards, AwardPot)
  - Handles timeouts with sensible defaults (auto-fold/check)

  # ==========================================================================
  # Hand Initialization
  # ==========================================================================
  # When a table starts a hand, the PM creates a HandProcess to track the
  # workflow. This process state persists across all phases of the hand.

  Scenario: Process manager initializes hand from HandStarted
    Given a HandFlowPM
    And a HandStarted event with:
      | hand_number | game_variant   | dealer_position | small_blind | big_blind |
      | 1           | TEXAS_HOLDEM   | 0               | 5           | 10        |
    And active players:
      | player_root | position | stack |
      | player-1    | 0        | 500   |
      | player-2    | 1        | 500   |
    When the process manager starts the hand
    Then a HandProcess is created with phase DEALING
    And the process has 2 players
    And the process has dealer_position 0

  # ==========================================================================
  # Blind Posting Phase
  # ==========================================================================
  # After cards are dealt, the PM drives blind posting: small blind first,
  # then big blind. Once both are posted, betting can begin.

  Scenario: Process manager transitions to blind posting after cards dealt
    Given an active hand process in phase DEALING
    And a CardsDealt event
    When the process manager handles the event
    Then the process transitions to phase POSTING_BLINDS
    And a PostBlind command is sent for small blind

  Scenario: Process manager posts big blind after small blind
    Given an active hand process in phase POSTING_BLINDS
    And small_blind_posted is true
    And a BlindPosted event for small blind
    When the process manager handles the event
    Then a PostBlind command is sent for big blind

  Scenario: Process manager starts betting after big blind posted
    Given an active hand process in phase POSTING_BLINDS
    And small_blind_posted is true
    And a BlindPosted event for big blind
    When the process manager handles the event
    Then the process transitions to phase BETTING
    And action_on is set to UTG position

  # ==========================================================================
  # Betting Round Management
  # ==========================================================================
  # The PM tracks who has acted, current bet, and when the round is complete.
  # A round ends when all active players have acted and matched the current bet.
  # Raises reset the "has acted" state for other players.

  Scenario: Process manager advances action after player acts
    Given an active hand process in phase BETTING
    And action_on is position 2
    And an ActionTaken event for player at position 2 with action CALL
    When the process manager handles the event
    Then action_on advances to next active player

  Scenario: Process manager resets has_acted after raise
    Given an active hand process in phase BETTING
    And players at positions 0, 1, 2 have all acted
    And an ActionTaken event for player at position 0 with action RAISE
    When the process manager handles the event
    Then players at positions 1 and 2 have has_acted reset to false

  Scenario: Process manager detects betting complete
    Given an active hand process in phase BETTING
    And all active players have acted and matched the current bet
    And an ActionTaken event for the last player
    When the process manager handles the event
    Then the betting round ends
    And the process advances to next phase

  Scenario: Process manager deals flop after preflop betting
    Given an active hand process with betting_phase PREFLOP
    And betting round is complete
    When the process manager ends the betting round
    Then a DealCommunityCards command is sent with count 3
    And the process transitions to phase DEALING_COMMUNITY

  Scenario: Process manager deals turn after flop betting
    Given an active hand process with betting_phase FLOP
    And betting round is complete
    When the process manager ends the betting round
    Then a DealCommunityCards command is sent with count 1

  Scenario: Process manager deals river after turn betting
    Given an active hand process with betting_phase TURN
    And betting round is complete
    When the process manager ends the betting round
    Then a DealCommunityCards command is sent with count 1

  Scenario: Process manager starts showdown after river betting
    Given an active hand process with betting_phase RIVER
    And betting round is complete
    When the process manager ends the betting round
    Then the process transitions to phase SHOWDOWN
    And an AwardPot command is sent

  # ==========================================================================
  # All-in and Early Endings
  # ==========================================================================
  # Hands can end early if all but one player folds. All-in players are
  # tracked separately since they can't take further actions but remain
  # eligible for pot awards.

  Scenario: Process manager awards pot to last player standing
    Given an active hand process with 2 players
    And an ActionTaken event with action FOLD
    When the process manager handles the event
    Then the process transitions to phase COMPLETE
    And an AwardPot command is sent to the remaining player

  Scenario: Process manager handles all-in correctly
    Given an active hand process in phase BETTING
    And an ActionTaken event with action ALL_IN
    When the process manager handles the event
    Then the player is marked as is_all_in
    And the player is not included in active players for betting

  # ==========================================================================
  # Timeout Handling
  # ==========================================================================
  # Players who don't act within the time limit are auto-acted: fold if
  # facing a bet, check if no bet. This prevents hands from stalling
  # indefinitely.

  Scenario: Process manager auto-folds on timeout when facing bet
    Given an active hand process in phase BETTING
    And current_bet is 20
    And action_on player has bet_this_round 0
    When the action times out
    Then the process manager sends PlayerAction with FOLD

  Scenario: Process manager auto-checks on timeout when no bet
    Given an active hand process in phase BETTING
    And current_bet is 0
    When the action times out
    Then the process manager sends PlayerAction with CHECK

  # ==========================================================================
  # Draw Game Phases
  # ==========================================================================
  # Draw games have an additional phase between betting rounds where players
  # discard and draw new cards. The PM tracks draw completion.

  Scenario: Process manager handles Five Card Draw phase transition
    Given an active hand process with game_variant FIVE_CARD_DRAW
    And betting_phase PREFLOP
    And betting round is complete
    When the process manager ends the betting round
    Then the process transitions to phase DRAW

  Scenario: Process manager starts final betting after draw
    Given an active hand process with game_variant FIVE_CARD_DRAW
    And betting_phase DRAW
    And all players have completed their draws
    When the process manager handles the last draw
    Then the process transitions to phase BETTING
    And betting_phase is set to DRAW

  # ==========================================================================
  # Community Card Dealing
  # ==========================================================================
  # When community cards are dealt, the PM resets betting state for the new
  # round: bet amounts reset to zero, action moves to first player after dealer.

  Scenario: Process manager resets betting state for new round
    Given an active hand process in phase BETTING
    And a CommunityCardsDealt event for FLOP
    When the process manager handles the event
    Then all players have bet_this_round reset to 0
    And all players have has_acted reset to false
    And current_bet is reset to 0
    And action_on is set to first player after dealer

  # ==========================================================================
  # State Management
  # ==========================================================================
  # The PM maintains accurate pot totals and player stacks throughout the
  # hand. These scenarios verify state updates are correct.

  Scenario: Process manager tracks pot total correctly
    Given an active hand process
    And a series of BlindPosted and ActionTaken events totaling 150
    When all events are processed
    Then pot_total is 150

  Scenario: Process manager tracks player stacks correctly
    Given an active hand process with player "player-1" at stack 500
    And an ActionTaken event for "player-1" with amount 50
    When the process manager handles the event
    Then "player-1" stack is 450

  Scenario: Process manager completes hand on PotAwarded
    Given an active hand process in phase SHOWDOWN
    And a PotAwarded event
    When the process manager handles the event
    Then the process transitions to phase COMPLETE
    And any pending timeout is cancelled
