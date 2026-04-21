Feature: Process Manager orchestration logic
  Process Managers coordinate cross-aggregate flows that require decision coupling.
  Unlike sagas which react to events after the fact, PMs validate state across
  aggregates BEFORE emitting commands, enabling synchronous client responses.

  # ==========================================================================
  # BuyInOrchestrator - Player ↔ Table coordination
  # ==========================================================================
  # The buy-in flow requires decision coupling: we need to know if the Table
  # seat is available BEFORE committing the Player's funds. The PM sees both
  # aggregate states and coordinates the atomic operation.

  Scenario: BuyInOrchestrator emits SeatPlayer when validation passes
    Given a table with seat 0 available and buy-in range 200-2000
    And a player with a BuyInRequested event for seat 0 with amount 500
    When the BuyInOrchestrator handles the BuyInRequested event
    Then the PM emits a SeatPlayer command to the table
    And the PM emits a BuyInInitiated process event

  Scenario: BuyInOrchestrator rejects when buy-in too low
    Given a table with seat 0 available and buy-in range 200-2000
    And a player with a BuyInRequested event for seat 0 with amount 100
    When the BuyInOrchestrator handles the BuyInRequested event
    Then the PM emits no commands
    And the PM emits a BuyInFailed process event with code "INVALID_AMOUNT"

  Scenario: BuyInOrchestrator rejects when buy-in too high
    Given a table with seat 0 available and buy-in range 200-2000
    And a player with a BuyInRequested event for seat 0 with amount 5000
    When the BuyInOrchestrator handles the BuyInRequested event
    Then the PM emits no commands
    And the PM emits a BuyInFailed process event with code "INVALID_AMOUNT"

  Scenario: BuyInOrchestrator rejects when seat is occupied
    Given a table with seat 0 occupied by another player
    And a player with a BuyInRequested event for seat 0 with amount 500
    When the BuyInOrchestrator handles the BuyInRequested event
    Then the PM emits no commands
    And the PM emits a BuyInFailed process event with code "SEAT_OCCUPIED"

  Scenario: BuyInOrchestrator rejects when table is full
    Given a table that is full with 9 players
    And a player with a BuyInRequested event for any seat with amount 500
    When the BuyInOrchestrator handles the BuyInRequested event
    Then the PM emits no commands
    And the PM emits a BuyInFailed process event with code "TABLE_FULL"

  Scenario: BuyInOrchestrator confirms buy-in on PlayerSeated
    Given a player and table in a pending buy-in state
    When the BuyInOrchestrator handles a PlayerSeated event
    Then the PM emits a ConfirmBuyIn command to the player
    And the PM emits a BuyInCompleted process event

  Scenario: BuyInOrchestrator releases funds on SeatingRejected
    Given a player and table in a pending buy-in state
    When the BuyInOrchestrator handles a SeatingRejected event
    Then the PM emits a ReleaseBuyIn command to the player
    And the PM emits a BuyInFailed process event with code "SEATING_REJECTED"

  # ==========================================================================
  # RegistrationOrchestrator - Player ↔ Tournament coordination
  # ==========================================================================
  # Tournament registration requires knowing capacity and status BEFORE
  # committing the Player's fee. Similar pattern to buy-in.

  Scenario: RegistrationOrchestrator emits EnrollPlayer when validation passes
    Given a tournament with registration open and capacity available
    And a player with a RegistrationRequested event with fee 1000
    When the RegistrationOrchestrator handles the RegistrationRequested event
    Then the PM emits an EnrollPlayer command to the tournament
    And the PM emits a RegistrationInitiated process event

  Scenario: RegistrationOrchestrator rejects when tournament is full
    Given a tournament that is full
    And a player with a RegistrationRequested event with fee 1000
    When the RegistrationOrchestrator handles the RegistrationRequested event
    Then the PM emits no commands
    And the PM emits a RegistrationFailed process event with code "REGISTRATION_CLOSED"

  Scenario: RegistrationOrchestrator rejects when registration is closed
    Given a tournament with registration closed
    And a player with a RegistrationRequested event with fee 1000
    When the RegistrationOrchestrator handles the RegistrationRequested event
    Then the PM emits no commands
    And the PM emits a RegistrationFailed process event with code "REGISTRATION_CLOSED"

  Scenario: RegistrationOrchestrator confirms on TournamentPlayerEnrolled
    Given a player and tournament in a pending registration state
    When the RegistrationOrchestrator handles a TournamentPlayerEnrolled event
    Then the PM emits a ConfirmRegistrationFee command to the player
    And the PM emits a RegistrationCompleted process event

  Scenario: RegistrationOrchestrator releases fee on TournamentEnrollmentRejected
    Given a player and tournament in a pending registration state
    When the RegistrationOrchestrator handles a TournamentEnrollmentRejected event
    Then the PM emits a ReleaseRegistrationFee command to the player
    And the PM emits a RegistrationFailed process event with code "ENROLLMENT_REJECTED"

  # ==========================================================================
  # RebuyOrchestrator - Player ↔ Tournament ↔ Table coordination
  # ==========================================================================
  # Rebuy is the most complex: requires validating Tournament rebuy window,
  # Player eligibility, AND Table seat existence. Three-domain coordination.

  Scenario: RebuyOrchestrator emits ProcessRebuy when all validations pass
    Given a tournament in rebuy window with player eligible
    And a table with the player seated at position 2
    And a player with a RebuyRequested event for amount 1000
    When the RebuyOrchestrator handles the RebuyRequested event
    Then the PM emits a ProcessRebuy command to the tournament
    And the PM emits a RebuyInitiated process event

  Scenario: RebuyOrchestrator rejects when rebuy window is closed
    Given a tournament with rebuy window closed
    And a table with the player seated at position 2
    And a player with a RebuyRequested event for amount 1000
    When the RebuyOrchestrator handles the RebuyRequested event
    Then the PM emits no commands
    And the PM emits a RebuyFailed process event with code "TOURNAMENT_NOT_RUNNING"

  Scenario: RebuyOrchestrator rejects when player not seated
    Given a tournament in rebuy window with player eligible
    And a table without the player seated
    And a player with a RebuyRequested event for amount 1000
    When the RebuyOrchestrator handles the RebuyRequested event
    Then the PM emits no commands
    And the PM emits a RebuyFailed process event with code "NOT_SEATED"

  Scenario: RebuyOrchestrator adds chips on RebuyProcessed
    Given a player, tournament, and table in a pending rebuy state
    When the RebuyOrchestrator handles a RebuyProcessed event
    Then the PM emits an AddRebuyChips command to the table

  Scenario: RebuyOrchestrator confirms fee after RebuyChipsAdded
    Given a player, tournament, and table with chips added
    When the RebuyOrchestrator handles a RebuyChipsAdded event
    Then the PM emits a ConfirmRebuyFee command to the player
    And the PM emits a RebuyCompleted process event

  Scenario: RebuyOrchestrator releases fee on RebuyDenied
    Given a player, tournament, and table in a pending rebuy state
    When the RebuyOrchestrator handles a RebuyDenied event
    Then the PM emits a ReleaseRebuyFee command to the player
    And the PM emits a RebuyFailed process event with code "REBUY_DENIED"
