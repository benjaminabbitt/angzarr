Feature: Fact injection from sagas and process managers

  Sagas and process managers can emit facts (events) that are injected
  directly into target aggregates without going through command handling.
  Facts ARE sequenced and persisted to the event store - they differ from
  commands only in that they bypass validation and cannot be rejected.

  Facts vs Commands vs Notifications:
  - Commands: Sequenced, validated, can be rejected
  - Facts: Sequenced, NOT validated, cannot be rejected
  - Notifications: NOT sequenced, used for coordination (e.g., compensation)

  Facts represent external realities:
  - Events that have already occurred elsewhere
  - State that the aggregate has no authority to reject
  - Cross-domain propagation where validation doesn't apply

  # ---------------------------------------------------------------------------
  # RequestAction: External reality from another aggregate
  # ---------------------------------------------------------------------------
  # When a hand determines it's a player's turn, that's a fact about the hand's
  # state. The player aggregate records this reality - it has no business
  # authority to reject "the hand says it's your turn."

  Scenario: Hand injects ActionRequested fact into player aggregate
    Given a registered player "Alice"
    And a hand in progress where it becomes Alice's turn
    When the hand-player saga processes the turn change
    Then an ActionRequested fact is injected into Alice's player aggregate
    And the fact is persisted with the next sequence number
    And the player aggregate contains an ActionRequested event

  Scenario: Fact receives sequence number from coordinator
    Given a player aggregate with 3 existing events
    When an ActionRequested fact is injected
    Then the fact is persisted with sequence number 4
    And subsequent events continue from sequence 5

  # ---------------------------------------------------------------------------
  # SitOut/SitIn: Player state propagated to table
  # ---------------------------------------------------------------------------
  # When a player decides to sit out, that's a fact about the player's intent.
  # The table aggregate records this reality - it cannot reject "player chose
  # to sit out."

  Scenario: Player sitting out is injected as fact to table
    Given player "Charlie" is seated at table "T1"
    When Charlie's player aggregate emits PlayerSittingOut
    Then a PlayerSatOut fact is injected into the table aggregate
    And the table records Charlie as sitting out
    And the fact has a sequence number in the table's event stream

  Scenario: Player sitting in is injected as fact to table
    Given player "Charlie" is sitting out at table "T1"
    When Charlie's player aggregate emits PlayerReturning
    Then a PlayerSatIn fact is injected into the table aggregate
    And the table records Charlie as active

  # ---------------------------------------------------------------------------
  # Fact metadata requirements
  # ---------------------------------------------------------------------------

  Scenario: Fact carries required metadata
    Given a saga that emits a fact
    When the fact is constructed
    Then the fact Cover has domain set to the target aggregate
    And the fact Cover has root set to the target aggregate root
    And the fact Cover has external_id set for idempotency
    And the fact Cover has correlation_id for traceability

  # ---------------------------------------------------------------------------
  # Error handling
  # ---------------------------------------------------------------------------

  Scenario: Fact injection failure fails the saga
    Given a saga that emits a fact to domain "nonexistent"
    When the saga processes an event
    Then the saga fails with error containing "not found"
    And no commands from that saga are executed

  Scenario: Duplicate fact with same external_id is idempotent
    Given a fact with external_id "action-H1-alice-turn-3"
    When the same fact is injected twice
    Then only one event is stored in the aggregate
    And the second injection succeeds without error
