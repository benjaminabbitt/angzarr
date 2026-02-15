@compensation
Feature: Compensation Flow - Components Handle Notification
  Aggregates and Process Managers handle Notification via @rejected
  decorated methods or on_rejected() fluent API to emit compensation events.

  Background:
    Given the angzarr framework is initialized

  # ============================================================================
  # Aggregate @rejected decorator - OO pattern
  # ============================================================================

  @handle @aggregate @oo
  Scenario: Aggregate @rejected handler dispatches by domain and command
    Given a Player aggregate with:
      | reserved_amount | 100 |
    And a @rejected handler for domain "payment" command "ProcessPayment"
    When the aggregate receives a Notification for:
      | domain  | payment        |
      | command | ProcessPayment |
    Then the @rejected handler is invoked
    And the handler receives the Notification
    And the handler can access aggregate state

  @handle @aggregate @oo
  Scenario: Aggregate @rejected handler emits compensation event
    Given a Player aggregate with:
      | player_root     | player-123 |
      | reserved_amount | 100        |
    And a @rejected handler that returns FundsReleased
    When the aggregate handles a payment rejection
    Then a FundsReleased event is emitted with:
      | player_root | player-123                  |
      | amount      | 100                         |
      | reason      | Payment failed: card_declined |

  @handle @aggregate @oo
  Scenario: Aggregate @rejected handler auto-applies returned events
    Given a Player aggregate with reserved_amount 100
    And a @rejected handler returning FundsReleased
    When the aggregate handles the rejection
    Then the FundsReleased event is applied to state
    And the aggregate reserved_amount becomes 0
    And the event is added to the event book

  @handle @aggregate @oo
  Scenario: Multiple @rejected handlers dispatch to correct one
    Given a Player aggregate with handlers:
      | domain    | command          | handler                    |
      | payment   | ProcessPayment   | handle_payment_rejected    |
      | inventory | ReserveItem      | handle_inventory_rejected  |
    When a rejection arrives for domain "inventory" command "ReserveItem"
    Then handle_inventory_rejected is called
    And handle_payment_rejected is not called

  @handle @aggregate @oo
  Scenario: No matching @rejected handler delegates to framework
    Given a Player aggregate with no @rejected handlers
    When the aggregate receives a Notification
    Then the response has emit_system_revocation true
    And the reason indicates no custom compensation

  # ============================================================================
  # Process Manager @rejected decorator - OO pattern
  # ============================================================================

  @handle @pm @oo
  Scenario: PM @rejected handler dispatches by domain and command
    Given an OrderWorkflowPM with:
      | order_id | order-123          |
      | step     | awaiting_inventory |
    And a @rejected handler for domain "inventory" command "ReserveInventory"
    When the PM receives a Notification for:
      | domain  | inventory         |
      | command | ReserveInventory  |
    Then the @rejected handler is invoked
    And the handler receives the Notification
    And the handler can access PM state

  @handle @pm @oo
  Scenario: PM @rejected handler emits PM domain events
    Given an OrderWorkflowPM with order_id "order-123"
    And a @rejected handler that returns WorkflowFailed
    When the PM handles an inventory rejection
    Then a WorkflowFailed event is recorded in PM state with:
      | order_id | order-123                            |
      | reason   | Inventory reservation failed: out_of_stock |
      | step     | inventory_reservation                |

  @handle @pm @oo
  Scenario: PM @rejected handler can trigger aggregate compensation
    Given an OrderWorkflowPM handling rejection
    When the PM @rejected handler completes
    Then the PM events are persisted
    And the framework continues to route to source aggregate

  @handle @pm @oo
  Scenario: No matching PM @rejected handler delegates to framework
    Given an OrderWorkflowPM with no @rejected handlers
    When the PM receives a Notification
    Then the PM returns no process events
    And emit_system_revocation is true

  # ============================================================================
  # CommandRouter.on_rejected() - Fluent pattern
  # ============================================================================

  @handle @aggregate @fluent
  Scenario: Fluent router on_rejected dispatches to handler
    Given a CommandRouter for domain "player" with:
      | on          | RegisterPlayer    | handle_register |
      | on_rejected | payment/ProcessPayment | handle_payment_rejected |
    When a Notification arrives with:
      | rejected_domain  | payment        |
      | rejected_command | ProcessPayment |
    Then handle_payment_rejected is called with:
      | notification | the Notification       |
      | state        | rebuilt player state   |

  @handle @aggregate @fluent
  Scenario: Fluent router on_rejected returns EventBook
    Given a CommandRouter with on_rejected handler
    And the handler returns an EventBook with FundsReleased
    When dispatch processes the Notification
    Then the BusinessResponse contains the EventBook
    And the EventBook has one FundsReleased event

  @handle @aggregate @fluent
  Scenario: Fluent router no matching on_rejected delegates to framework
    Given a CommandRouter with no on_rejected handlers
    When dispatch processes a Notification
    Then the BusinessResponse has revocation
    And emit_system_revocation is true
    And reason contains "no custom compensation"

  @handle @aggregate @fluent
  Scenario: Multiple on_rejected handlers in fluent chain
    Given a CommandRouter configured as:
      """
      router = CommandRouter("player", rebuild)
        .on("RegisterPlayer", handle_register)
        .on_rejected("payment", "ProcessPayment", handle_payment)
        .on_rejected("inventory", "ReserveItem", handle_inventory)
      """
    When a rejection arrives for "inventory/ReserveItem"
    Then handle_inventory is called
    And handle_payment is not called

  # ============================================================================
  # Dispatch key extraction
  # ============================================================================

  @handle @dispatch
  Scenario: Dispatch key extracted from rejected_command.cover.domain
    Given a Notification with rejected_command:
      | cover.domain | payment |
    When the router extracts the dispatch key
    Then the domain part is "payment"

  @handle @dispatch
  Scenario: Dispatch key extracted from rejected_command type_url
    Given a Notification with rejected_command:
      | type_url | type.googleapis.com/myapp.ProcessPayment |
    When the router extracts the dispatch key
    Then the command part is "ProcessPayment"

  @handle @dispatch
  Scenario: Full dispatch key is domain/command
    Given a Notification with:
      | cover.domain | payment                                  |
      | type_url     | type.googleapis.com/myapp.ProcessPayment |
    When the router builds the dispatch key
    Then the key is "payment/ProcessPayment"

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
    Given an OrderWorkflowPM with prior events:
      | event             | data                |
      | WorkflowStarted   | order_id: order-123 |
      | InventoryRequested| quantity: 5         |
    When a rejection handler accesses state
    Then order_id is "order-123"
    And step is "awaiting_inventory"

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
    Given an OrderWorkflowPM in step "awaiting_inventory"
    And a @rejected handler that emits StepRolledBack
    When compensation completes
    Then the PM step changes to "inventory_failed"
    And the PM can transition to a recovery path
