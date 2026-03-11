Feature: Synchronous Processing Modes
  Angzarr supports three synchronous processing modes that control how events
  propagate through the system after a command executes. Choosing the right
  mode balances latency, consistency, and failure handling.

  Why sync modes matter:
  - ASYNC (default): Fastest response, eventual consistency. Use for high-throughput
    scenarios where immediate cross-domain visibility isn't required.
  - SIMPLE: Projectors run synchronously, sagas/PMs run asynchronously. Use when
    read models must be immediately consistent but cross-domain effects can lag.
  - CASCADE: Full synchronous propagation through sagas and PMs. Use when the
    response must reflect all cross-domain effects (expensive, use sparingly).

  Patterns exercised by these tests:
  - Event-driven architecture: Events trigger projectors, sagas, and PMs
  - Read model consistency: Projectors update read models from events
  - Cross-domain coordination: Sagas translate events into commands for other domains
  - Error propagation: CASCADE mode surfaces downstream failures immediately

  Why poker exercises sync modes well:
  - Clear read models: Player bankroll projections show immediate vs delayed updates
  - Cross-domain flows: Table→Hand→Player flows test cascade depth
  - Measurable latency: "within N seconds" assertions verify async vs sync behavior
  - Observable side effects: Fund reservations show saga execution timing

  Background:
    Given the poker system is running in standalone mode

  # ===========================================================================
  # ASYNC Mode - Fire and Forget
  # ===========================================================================
  # ASYNC mode persists events and publishes to the bus immediately, without
  # waiting for projectors or sagas. Fastest mode, but read models may lag.

  @sync-mode @async
  Scenario: ASYNC mode returns before projectors complete
    Given registered players with bankroll:
      | name  | bankroll |
      | Alice | 1000     |
    When I deposit 500 chips to player "Alice" with sync_mode ASYNC
    Then the command succeeds immediately
    And the response does not include projection updates
    But within 2 seconds player "Alice" bankroll projection shows 1500

  @sync-mode @async
  Scenario: ASYNC mode does not wait for saga execution
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When I start a hand at table "Main" with sync_mode ASYNC
    Then the command succeeds with HandStarted event
    And the response does not include cascade results
    But within 3 seconds hand domain has CardsDealt event

  # ===========================================================================
  # SIMPLE Mode - Sync Projectors, Async Sagas
  # ===========================================================================
  # SIMPLE mode waits for projectors to complete before returning, ensuring
  # read models are immediately consistent. Sagas and PMs run asynchronously.

  @sync-mode @simple
  Scenario: SIMPLE mode includes projection updates in response
    Given registered players with bankroll:
      | name  | bankroll |
      | Alice | 1000     |
    When I deposit 500 chips to player "Alice" with sync_mode SIMPLE
    Then the command succeeds
    And the response includes projection updates for "output-projector"
    And the projection shows bankroll 1500

  @sync-mode @simple
  Scenario: SIMPLE mode projectors see events immediately
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When I start a hand at table "Main" with sync_mode SIMPLE
    Then the response includes projection updates
    And the table projection shows hand_count incremented
    But the response does not include cascade results from sagas

  @sync-mode @simple
  Scenario: SIMPLE mode saga execution is asynchronous
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When I start a hand at table "Main" with sync_mode SIMPLE
    Then the command returns before DealCards is issued
    But within 3 seconds hand domain has CardsDealt event

  # ===========================================================================
  # CASCADE Mode - Full Synchronous Propagation
  # ===========================================================================
  # CASCADE mode executes projectors, sagas, and PMs synchronously, recursively
  # following the event chain until completion. Response includes all results.

  @sync-mode @cascade
  Scenario: CASCADE mode waits for full saga chain
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When I start a hand at table "Main" with sync_mode CASCADE
    Then the response includes cascade results
    And the cascade results include DealCards command to hand domain
    And the cascade results include CardsDealt event from hand domain

  @sync-mode @cascade
  Scenario: CASCADE mode includes all projection updates
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When I start a hand at table "Main" with sync_mode CASCADE
    Then the response includes projection updates for both table and hand domains

  @sync-mode @cascade
  Scenario: CASCADE mode follows multi-hop saga chains
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And a hand is in progress with "Alice" to act
    When "Alice" folds with sync_mode CASCADE
    Then the response includes the full cascade chain:
      | domain | event_type    |
      | hand   | PlayerFolded  |
      | hand   | HandComplete  |
      | table  | HandEnded     |
      | player | FundsReleased |

  @sync-mode @cascade
  Scenario: CASCADE mode does not publish to event bus
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And I am monitoring the event bus
    When I start a hand at table "Main" with sync_mode CASCADE
    Then no events are published to the bus during command execution
    And all events remain in-process

  # ===========================================================================
  # CascadeErrorMode - Error Handling in CASCADE
  # ===========================================================================
  # When CASCADE mode encounters errors in downstream sagas or PMs, the
  # cascade_error_mode determines how to proceed.

  @cascade-error @fail-fast
  Scenario: FAIL_FAST stops on first saga error
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And the table-hand saga is configured to fail
    When I start a hand at table "Main" with sync_mode CASCADE and cascade_error_mode FAIL_FAST
    Then the command fails with saga error
    And no further sagas are executed after the failure
    And the original HandStarted event is still persisted

  @cascade-error @continue
  Scenario: CONTINUE collects errors and proceeds
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And the table-hand saga is configured to fail
    And the output projector is healthy
    When I start a hand at table "Main" with sync_mode CASCADE and cascade_error_mode CONTINUE
    Then the command succeeds
    And the response includes cascade_errors with the saga failure
    And the response includes successful projection updates
    And other sagas continue executing despite the failure

  @cascade-error @compensate
  Scenario: COMPENSATE rolls back on saga error
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And the hand-player saga is configured to fail on PotAwarded
    And a hand is in progress
    When the hand completes with sync_mode CASCADE and cascade_error_mode COMPENSATE
    Then compensation commands are issued in reverse order
    And the command fails after compensation completes

  @cascade-error @dead-letter
  Scenario: DEAD_LETTER sends failures to DLQ and continues
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And the table-hand saga is configured to fail
    And a dead letter queue is configured
    When I start a hand at table "Main" with sync_mode CASCADE and cascade_error_mode DEAD_LETTER
    Then the command succeeds
    And the saga failure is published to the dead letter queue
    And the dead letter includes:
      | field             | value             |
      | source_component  | saga-table-hand   |
      | rejection_reason  | contains error    |
    And other sagas continue executing

  # ===========================================================================
  # Process Manager Sync Behavior
  # ===========================================================================
  # Process managers in CASCADE mode maintain correlation across domains.

  @sync-mode @cascade @pm
  Scenario: CASCADE mode includes process manager execution
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And the hand-flow process manager is registered
    When I start a hand at table "Main" with sync_mode CASCADE
    Then the process manager receives the correlated events
    And the response includes PM state updates

  @sync-mode @cascade @pm
  Scenario: Process manager skipped without correlation ID
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    And the hand-flow process manager is registered
    When I send an event without correlation_id with sync_mode CASCADE
    Then the process manager is not invoked
    And sagas still execute normally

  # ===========================================================================
  # Performance Characteristics
  # ===========================================================================
  # Different modes have different latency profiles.

  @sync-mode @performance
  Scenario: ASYNC mode has lowest latency
    Given 10 registered players
    When I deposit chips to all players with sync_mode ASYNC
    Then all commands complete within 100ms each
    And total execution time is less than with SIMPLE mode

  @sync-mode @performance
  Scenario: CASCADE mode has highest latency but full consistency
    Given a table "Main" with seated players:
      | name  | seat | stack |
      | Alice | 0    | 500   |
      | Bob   | 1    | 500   |
    When I start a hand at table "Main" with sync_mode CASCADE
    Then the response time is higher than ASYNC or SIMPLE
    But all cross-domain state is consistent immediately

  # ===========================================================================
  # Edge Cases
  # ===========================================================================

  @sync-mode @edge-case
  Scenario: Empty saga list in CASCADE mode succeeds
    Given a domain with no registered sagas
    When I execute a command with sync_mode CASCADE
    Then the command succeeds
    And the response has empty cascade_results

  @sync-mode @edge-case
  Scenario: Saga producing no commands in CASCADE mode
    Given a table with no seated players
    When I start a hand at table "Empty" with sync_mode CASCADE
    Then the saga produces no commands
    And the command succeeds with HandStarted only

  @cascade-error @edge-case
  Scenario: All sagas fail in CONTINUE mode
    Given multiple sagas configured to fail
    When I execute a triggering command with cascade_error_mode CONTINUE
    Then the command succeeds
    And all saga errors are collected in cascade_errors
    And the original event is still persisted
