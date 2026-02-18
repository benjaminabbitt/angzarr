Feature: PositionStore interface
  Handlers (projectors, sagas, process managers) must remember where they left
  off. The PositionStore records the last-processed sequence per handler, per
  aggregate. On restart, handlers resume from their checkpoint instead of
  reprocessing the entire event history.

  Background:
    Given a PositionStore backend

  # ==========================================================================
  # Position Retrieval
  # ==========================================================================

  Scenario: First-time handlers start from the beginning
    Given a handler "player-projector" tracking domain "player"
    When I get the position for the handler
    Then the position should not exist

  # ==========================================================================
  # Position Updates
  # ==========================================================================

  Scenario: Handlers checkpoint their progress for crash recovery
    Given a handler "player-projector" tracking domain "player"
    When I put position 42 for the handler
    And I get the position for the handler
    Then the position should be 42

  Scenario: Checkpoints always reflect the latest progress
    Given a handler "player-projector" tracking domain "player"
    When I put position 10 for the handler
    And I put position 25 for the handler
    And I get the position for the handler
    Then the position should be 25

  Scenario: Sequence zero is a valid checkpoint
    Given a handler "player-projector" tracking domain "player"
    When I put position 0 for the handler
    And I get the position for the handler
    Then the position should be 0

  # ==========================================================================
  # Handler Isolation
  # ==========================================================================

  Scenario: Different handlers track progress independently
    Given a handler "player-projector" tracking domain "player"
    And a handler "output-projector" tracking domain "player"
    When I put position 10 for handler "player-projector"
    And I put position 20 for handler "output-projector"
    Then the position for handler "player-projector" should be 10
    And the position for handler "output-projector" should be 20

  # ==========================================================================
  # Domain Isolation
  # ==========================================================================

  Scenario: Multi-domain handlers track each domain independently
    Given a handler "hand-table-saga" tracking domain "hand"
    And a handler "hand-table-saga" tracking domain "table"
    When I put position 5 for domain "hand"
    And I put position 15 for domain "table"
    Then the position for domain "hand" should be 5
    And the position for domain "table" should be 15

  # ==========================================================================
  # Root Isolation
  # ==========================================================================

  Scenario: Each aggregate root has its own checkpoint
    Given a handler "player-projector" tracking domain "player" with root "player-1"
    And a handler "player-projector" tracking domain "player" with root "player-2"
    When I put position 100 for root "player-1"
    And I put position 200 for root "player-2"
    Then the position for root "player-1" should be 100
    And the position for root "player-2" should be 200

  # ==========================================================================
  # Concurrent Handler Scaling
  # ==========================================================================

  Scenario: Scaled-out handler instances don't interfere
    Given 5 handlers tracking domain "player" with root "shared-root"
    When each handler puts its index times 10 as position
    Then each handler should have position equal to its index times 10
