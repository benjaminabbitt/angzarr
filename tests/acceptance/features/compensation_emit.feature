@compensation
Feature: Compensation Flow - Framework Emits Notification
  When a saga or process manager issues a command that gets rejected by the
  target aggregate, the system must notify the original components so they
  can compensate (undo partial work, update their state, or retry).

  Why automatic notification matters:
  - Sagas are stateless - they can't poll for rejection status
  - The source aggregate needs to know its triggered action failed
  - Process managers need to update their workflow state
  - Without notification, partial operations would remain uncommitted forever

  Patterns enabled by compensation notification:
  - Distributed saga rollback: Each step in a multi-step saga can be undone
    when a downstream step fails. Same pattern applies to payment→fulfillment.
  - Workflow state tracking: PMs record which step failed for reporting and
    retry logic. Same pattern applies to approval chains, onboarding flows.
  - Source aggregate cleanup: The originating aggregate releases held resources.
    Same pattern applies to inventory holds, payment authorizations.

  Why poker exercises compensation patterns well:
  - FundsReserved→JoinTable failure is obvious: $500 was held, now must be released
  - Multi-step coordination: Table→Hand→Player with clear failure points
  - Visible compensation: FundsReleased event is explicit, auditable
  - Multiple failure scenarios: table full, insufficient buy-in, hand rejected

  The flow:
  1. Saga/PM issues command (triggered by source event)
  2. Target aggregate rejects with FAILED_PRECONDITION
  3. Framework wraps rejection in Notification message
  4. Framework routes Notification back through the chain:
     - To PM first (if PM issued the command) - can update workflow state
     - To source aggregate - can emit compensation events

  Poker example: Player reserves $500 to join a table. The table-player-saga
  issues JoinTable to the table aggregate. If the table is full (rejection),
  the player needs to release those reserved funds (compensation).

  Background:
    Given the angzarr framework is initialized

  # ============================================================================
  # Saga-issued command rejected
  # ============================================================================
  # Sagas translate events from domain A into commands for domain B.
  # When domain B rejects the command, domain A needs to compensate.

  @emit @saga
  Scenario: Saga rejection triggers Notification creation
    # Player reserves funds → table-player-saga issues JoinTable → Table rejects.
    # The saga is stateless and won't know about the rejection without notification.
    Given a Player aggregate that emitted FundsReserved (money set aside for table buy-in)
    And a table-player-saga listening for FundsReserved, issuing JoinTable to table domain
    When the Table aggregate rejects JoinTable with "table_full"
    Then the framework creates a Notification containing:
      | issuer_name     | saga-table-player | # who issued the failed command
      | rejection_reason| table_full        | # why it failed
      | rejected_command| JoinTable         | # what failed
    # This Notification will be routed back to Player for compensation

  @emit @saga
  Scenario: Notification routes back to the source aggregate
    # The source aggregate (Player) needs to know its action's downstream effect failed.
    # It emitted FundsReserved, which triggered a saga, which failed.
    # Player must release those reserved funds.
    Given Player emitted FundsReserved → table-player-saga issued JoinTable → rejected
    When the framework routes the rejection
    Then Player receives the Notification
    And the notification identifies source_event_type "FundsReserved"
    # Player can now emit FundsReleased to compensate

  @emit @saga
  Scenario: Notification preserves full command context for debugging
    # When investigating failures, operators need complete context:
    # Which aggregate? Which command? What correlation ID traces the workflow?
    Given a saga command targeting player-123 with correlation_id corr-456
    When the command is rejected with reason "table_full"
    Then the Notification contains the full rejected command
    And includes cover.domain, cover.root for routing
    And includes rejection_reason for compensation logic decisions

  # ============================================================================
  # PM-issued command rejected
  # ============================================================================
  # Process managers coordinate multi-domain workflows. When a step fails,
  # the PM needs to update its workflow state AND the source aggregate needs
  # to compensate. The framework routes to both.

  @emit @pm
  Scenario: PM rejection creates Notification with PM identity
    # HandFlowPM coordinates: Table → Hand → Player balance updates
    # If Hand aggregate rejects a command, the PM needs to mark the step failed.
    Given Table emitted HandStarted
    And HandFlowPM reacts by issuing DealCards to Hand
    When Hand rejects with "invalid_player_count"
    Then a Notification is created identifying:
      | issuer_name     | pmg-hand-flow        | # the PM that issued the command
      | rejection_reason| invalid_player_count | # why Hand refused
    # The PM can now transition to a "failed" or "retry" state

  @emit @pm
  Scenario: PM receives Notification before source aggregate
    # PM state must update first so it can:
    # - Record the failure step
    # - Decide if retry is possible
    # - Coordinate any multi-step rollback
    # THEN the source aggregate compensates.
    Given Table → HandFlowPM → Hand (rejected)
    When the framework routes the rejection
    Then HandFlowPM receives the Notification first
    And can update its workflow state (e.g., step = "deal_failed")
    Then Table receives the Notification second
    And can emit compensation events (e.g., HandCancelled)

  @emit @pm
  Scenario: Notification links back to PM's correlation context
    # The PM tracks workflow state by correlation_id. The Notification must
    # carry enough context to match it back to the right workflow instance.
    Given HandFlowPM tracking hand-789 at step "awaiting_deal"
    When its DealCards command is rejected
    Then the Notification includes correlation_id linking to this PM instance
    And the PM can load its state to make compensation decisions

  # ============================================================================
  # Rejection triggers
  # ============================================================================
  # Not all errors trigger compensation. Only business rejections do.

  @emit
  Scenario: Business rejection (FAILED_PRECONDITION) triggers compensation
    # FAILED_PRECONDITION = "I understood your request but can't fulfill it"
    # Examples: insufficient balance, item out of stock, user not authorized
    # These are recoverable business conditions, not bugs.
    Given a saga issues a command to an aggregate
    When the aggregate returns gRPC FAILED_PRECONDITION
    Then the framework creates a Notification
    And routes it for compensation
    # The source aggregate can emit events to undo partial work

  @emit
  Scenario: Input validation errors do not trigger compensation
    # INVALID_ARGUMENT = "your request is malformed"
    # This is a bug in the caller, not a business condition.
    # No compensation needed - nothing was partially done.
    Given a saga issues a malformed command
    When the aggregate returns gRPC INVALID_ARGUMENT
    Then no Notification is created
    And the error propagates to the original caller
    # Fix the bug; don't try to compensate for invalid requests

  @emit
  Scenario: Notification captures full provenance for debugging
    # When investigating "why did this hand fail to start?", operators need the
    # complete chain: which event triggered which component to issue what.
    Given this chain of events:
      | step | component        | action               |
      | 1    | Table aggregate  | emits HandStarted    |
      | 2    | HandFlowPM       | issues DealCards     |
      | 3    | Hand aggregate   | rejects (bad config) |
    When the framework creates the Notification
    Then it contains the full provenance:
      | source_event_type | HandStarted    | # what started the chain
      | rejected_command  | DealCards      | # what failed
      | issuer_type       | process_manager| # saga vs PM
      | issuer_name       | pmg-hand-flow  | # which component

  # ============================================================================
  # Edge cases
  # ============================================================================
  # Complex scenarios that test compensation routing correctness.

  @emit @edge
  Scenario: Multi-command saga stops on first rejection
    # A saga might issue: ReserveFunds, then JoinTable.
    # If funds fail, we must NOT try to join - stop immediately.
    # Only one Notification is created (for the funds failure).
    Given a saga that issues commands sequentially:
      | 1. ReserveFunds | → player (might fail)    |
      | 2. JoinTable    | → table (never sent)     |
    When ReserveFunds is rejected (insufficient_balance)
    Then JoinTable is never issued
    And exactly one Notification is created (for funds rejection)
    # This prevents seating players who can't afford the buy-in

  @emit @edge
  Scenario: Nested PM chain bubbles rejection through all levels
    # PM chains can coordinate complex flows: HandFlowPM → nested workflows
    # If a downstream aggregate rejects, each PM in the chain must be notified to
    # update their workflow state, then the original aggregate compensates.
    Given a PM chain: HandFlowPM → nested-workflow → Hand
    When Hand rejects AwardPot command
    Then Notifications route through the chain in reverse:
      | 1. nested-workflow | updates workflow state      |
      | 2. HandFlowPM      | updates workflow state      |
      | 3. Source aggregate| emits compensation events   |
    # Each PM records the failure before passing notification upstream
