@compensation
Feature: Compensation Flow - Components Handle Notification
  When a Notification arrives (indicating a downstream command was rejected),
  the source component must compensate - typically by emitting events that
  undo the partial work that triggered the failed command.

  Why components handle their own compensation:
  - Business logic knows what to undo (release reserved funds, cancel pending action)
  - Compensation may vary by rejection reason (retry vs cancel vs escalate)
  - Framework can't know domain-specific rollback semantics

  Patterns enabled by component compensation handling:
  - Domain-specific rollback: Only the aggregate knows how to undo its operations.
    FundsReserved→FundsReleased. Same pattern applies to inventory→release.
  - Reason-based branching: Different rejection reasons may need different
    compensation. "table_full" might retry; "banned_player" might not.
  - State-aware compensation: Handler can access current state to calculate
    correct compensation amount. Same pattern applies to partial refunds.

  Why poker exercises compensation handling well:
  - Clear undo semantics: FundsReserved has exactly one undo: FundsReleased
  - Amount tracking: Handler must release the exact reserved amount
  - Multiple rejection reasons: table_full, insufficient_buy_in, player_banned
  - PM + aggregate chain: PM updates workflow state, then aggregate compensates

  Two patterns for handling:
  - @rejected decorator (OO): method annotated with domain/command it handles
  - on_rejected() fluent API: functional style for simpler aggregates

  If no handler matches, the framework emits a generic revocation event,
  which may be insufficient for complex business compensation.

  Poker example: Player reserves $500 for table buy-in → JoinTable rejected (table full)
  → Player receives Notification → Player emits FundsReleased to restore available balance

  Background:
    Given the angzarr framework is initialized

  # ============================================================================
  # Aggregate @rejected decorator - OO pattern
  # ============================================================================
  # The @rejected decorator routes Notifications to the right handler based
  # on which domain/command was rejected. The handler emits compensation events.

  @handle @aggregate @oo
  Scenario: Rejection routes to handler matching domain and command
    # Player reserved $500 for table buy-in. table-player-saga issued JoinTable.
    # Table aggregate rejected (table full). Player must release the $500.
    # The @rejected handler for "table/JoinTable" is invoked.
    Given Player has reserved_amount 500 (funds set aside for table buy-in)
    And a @rejected handler registered for domain "table" command "JoinTable"
    When Player receives Notification for table/JoinTable rejection
    Then the matching @rejected handler is invoked
    And receives the Notification (with rejection reason, failed command)
    And can access current aggregate state (to calculate compensation)

  @handle @aggregate @oo
  Scenario: Handler emits events to compensate for the failed operation
    # The handler's job: emit events that undo the partial work.
    # FundsReserved set aside $500 → JoinTable failed → FundsReleased gives it back.
    Given Player with reserved_amount 500
    And a @rejected handler that returns FundsReleased event
    When the handler processes a table rejection (reason: table_full)
    Then FundsReleased is emitted with:
      | amount | 500                     |
      | reason | Join failed: table_full |
    # Player's available balance is restored

  @handle @aggregate @oo
  Scenario: Compensation events are applied and persisted atomically
    # Events returned by @rejected handlers are both applied to state
    # AND persisted - same as events from regular command handlers.
    Given Player with reserved_amount 100
    When @rejected handler returns FundsReleased
    Then state.reserved_amount becomes 0 (event applied)
    And FundsReleased is added to the event book (persisted)
    # State and events are consistent

  @handle @aggregate @oo
  Scenario: Multiple handlers route to the correct one by domain/command
    # An aggregate may participate in multiple workflows, each potentially failing.
    # Table join failure needs different compensation than hand start failure.
    Given Player has @rejected handlers for:
      | domain | command   | handler               |
      | table  | JoinTable | handle_join_rejected  |
      | hand   | PostBlind | handle_blind_rejected |
    When a rejection arrives for table/JoinTable
    Then handle_join_rejected is called (correct handler)
    And handle_blind_rejected is NOT called (wrong domain)

  @handle @aggregate @oo
  Scenario: Missing handler delegates compensation to framework
    # If no custom handler exists, the framework emits a generic revocation.
    # This may be insufficient for business needs but prevents silent failures.
    Given Player has no @rejected handlers configured
    When Player receives a Notification
    Then response.emit_system_revocation = true
    And reason indicates "no custom compensation handler"
    # Developer should add a handler for proper business compensation

  # ============================================================================
  # Process Manager @rejected decorator - OO pattern
  # ============================================================================
  # PMs receive Notifications BEFORE the source aggregate. This lets them
  # update workflow state (mark step failed, decide retry) before compensation.

  @handle @pm @oo
  Scenario: PM rejection routes to handler matching the failed command
    # HandFlowPM tracks: Table → Hand → Player balance updates
    # If Hand fails to deal, the PM must record which step failed.
    Given HandFlowPM at step "awaiting_deal" for hand-123
    And a @rejected handler for domain "hand" command "DealCards"
    When PM receives Notification for hand/DealCards rejection
    Then the matching @rejected handler is invoked
    And receives the Notification (with rejection details)
    And can access PM state (current step, hand_id, history)

  @handle @pm @oo
  Scenario: PM handler emits workflow events (not aggregate events)
    # PMs have their own event stream tracking workflow progress.
    # On failure, emit WorkflowFailed or StepFailed - not domain events.
    Given HandFlowPM tracking hand-123
    And @rejected handler returning WorkflowFailed
    When PM handles hand rejection (invalid_player_count)
    Then WorkflowFailed is recorded in PM's event stream:
      | hand_id | hand-123                                 |
      | reason  | Deal failed: invalid_player_count        |
      | step    | deal_cards                               |
    # This records WHY the workflow failed for debugging/reporting

  @handle @pm @oo
  Scenario: After PM handles rejection, framework routes to source aggregate
    # PM updates workflow state, then the chain continues.
    # Source aggregate still needs to emit its compensation events.
    Given HandFlowPM handles a rejection
    When PM @rejected handler completes
    Then PM events are persisted (workflow state updated)
    And framework routes Notification to source aggregate next
    # PM recorded failure; now aggregate compensates

  @handle @pm @oo
  Scenario: PM without handler delegates to framework
    # If PM has no custom handler, framework handles it generically.
    # PM state won't be updated, which may leave workflow in inconsistent state.
    Given HandFlowPM with no @rejected handlers
    When PM receives a Notification
    Then PM returns no process events (state unchanged)
    And emit_system_revocation = true
    # Developer should add handler to update workflow state properly

  # ============================================================================
  # CommandRouter.on_rejected() - Fluent pattern
  # ============================================================================
  # Alternative to @rejected decorator. Same routing logic, functional style.
  # Useful for simpler aggregates or when decorators feel heavyweight.

  @handle @aggregate @fluent
  Scenario: Fluent API routes rejections to registered handlers
    # Same routing as @rejected, but configured via method chaining.
    Given CommandRouter for "player" domain configured with:
      | on          | RegisterPlayer    | handle_register      |
      | on_rejected | table/JoinTable   | handle_join_rejected |
    When Notification arrives for table/JoinTable rejection
    Then handle_join_rejected receives:
      | notification | the Notification details |
      | state        | current player state     |
    # Handler can inspect state to decide compensation

  @handle @aggregate @fluent
  Scenario: Fluent handler returns EventBook with compensation events
    # Handler builds an EventBook containing compensation events.
    # Framework persists and applies them.
    Given CommandRouter with on_rejected handler
    And handler builds EventBook containing FundsReleased
    When router dispatches the Notification
    Then BusinessResponse contains the EventBook
    And FundsReleased will be persisted and applied

  @handle @aggregate @fluent
  Scenario: No matching fluent handler delegates to framework
    # Same as OO pattern - missing handler triggers generic revocation.
    Given CommandRouter with no on_rejected handlers
    When router dispatches a Notification
    Then BusinessResponse has emit_system_revocation = true
    And reason: "no custom compensation handler"

  @handle @aggregate @fluent
  Scenario: Multiple on_rejected handlers in fluent chain
    # Chain multiple handlers for different failure scenarios.
    Given CommandRouter configured as:
      """
      CommandRouter("player", rebuild)
        .on("RegisterPlayer", handle_register)
        .on_rejected("table", "JoinTable", handle_join)
        .on_rejected("hand", "PostBlind", handle_blind)
      """
    When rejection arrives for table/JoinTable
    Then handle_join is called (matches)
    And handle_blind is NOT called (different domain/command)

  # ============================================================================
  # Dispatch key extraction
  # ============================================================================
  # The router must extract domain/command from the Notification to find
  # the right handler. This tests the key extraction logic.

  @handle @dispatch
  Scenario: Domain extracted from rejected command's cover
    # The cover.domain identifies which bounded context rejected.
    Given Notification with rejected_command.cover.domain = "table"
    When router extracts dispatch key
    Then domain part = "table"

  @handle @dispatch
  Scenario: Command type extracted from type_url
    # type_url is "type.googleapis.com/package.CommandName"
    # We extract just the CommandName part.
    Given Notification with rejected_command.type_url = "type.googleapis.com/examples.JoinTable"
    When router extracts dispatch key
    Then command part = "JoinTable"

  @handle @dispatch
  Scenario: Full dispatch key combines domain and command
    # Handlers register for "domain/command" pairs.
    # The router builds this key from the Notification.
    Given Notification with domain "table" and command "JoinTable"
    When router builds dispatch key
    Then key = "table/JoinTable"
    # This key matches handler registered for table/JoinTable

  # ============================================================================
  # Compensation event recording
  # ============================================================================

  @handle @events
  Scenario: Compensation events have proper metadata
    Given a Player aggregate handling rejection
    When a FundsReleased compensation event is emitted
    Then the event has a created_at timestamp
    And the event has the correct sequence number
    And the event is packed as Any with proper type_url

  @handle @events
  Scenario: Multiple compensation events in single handler
    Given an aggregate @rejected handler returning multiple events:
      | event              |
      | ReservationCancelled |
      | RefundIssued         |
    When the handler completes
    Then both events are in the EventBook
    And the events have sequential sequence numbers

  # ============================================================================
  # State access during compensation
  # ============================================================================

  @handle @state
  Scenario: Aggregate state is rebuilt before rejection handling
    Given a Player aggregate with prior events:
      | event          | amount |
      | FundsDeposited | 500    |
      | FundsReserved  | 100    |
    When a rejection handler accesses state.balance
    Then the balance is 400
    And reserved_amount is 100

  @handle @state
  Scenario: PM state is rebuilt before rejection handling
    Given a HandFlowPM with prior events:
      | event           | data               |
      | WorkflowStarted | hand_id: hand-123  |
      | DealRequested   | player_count: 3    |
    When a rejection handler accesses state
    Then hand_id is "hand-123"
    And step is "awaiting_deal"

  # ============================================================================
  # Edge cases and error handling
  # ============================================================================

  @handle @edge
  Scenario: Handler throws error during compensation
    Given a @rejected handler that raises an exception
    When the aggregate handles the Notification
    Then the exception propagates
    And no compensation events are persisted
    And the framework can retry or escalate

  @handle @edge
  Scenario: Empty rejected_command in Notification
    Given a malformed Notification with no rejected_command
    When the router attempts dispatch
    Then the dispatch key is empty
    And no handler matches
    And framework delegation occurs

  @handle @edge
  Scenario: Handler returns None/empty
    Given a @rejected handler that returns None
    When the aggregate handles the rejection
    Then no events are added to the event book
    And the response still indicates success

  # ============================================================================
  # Integration with saga retry
  # ============================================================================

  @handle @retry
  Scenario: Compensation enables saga retry
    Given a saga with retry logic
    And the first attempt rejected with "temporary_failure"
    When compensation completes successfully
    Then the saga can retry the operation
    And the retry has fresh state

  @handle @retry
  Scenario: PM updates step after compensation
    Given a HandFlowPM in step "awaiting_deal"
    And a @rejected handler that emits StepRolledBack
    When compensation completes
    Then the PM step changes to "deal_failed"
    And the PM can transition to a recovery path
