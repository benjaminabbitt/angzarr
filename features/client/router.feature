# docs:start:router_contract
Feature: Router - Command and Event Routing
  Routers dispatch incoming commands/events to appropriate handlers.
  Each component type (aggregate, saga, projector, PM) has its own
  router pattern, but all share common routing concepts.

  Routers enable:
  - Type-based dispatch to handlers
  - State reconstruction before handling
  - Event emission after handling
  - Error handling and compensation
# docs:end:router_contract

  # ==========================================================================
  # Aggregate Router
  # ==========================================================================

  Scenario: Aggregate router dispatches by command type
    Given an aggregate router with handlers for "CreateOrder" and "AddItem"
    When I receive a "CreateOrder" command
    Then the CreateOrder handler should be invoked
    And the AddItem handler should NOT be invoked

  Scenario: Aggregate router loads state before handling
    Given an aggregate router
    And an aggregate with existing events
    When I receive a command for that aggregate
    Then the router should load the EventBook first
    And the handler should receive the reconstructed state

  Scenario: Aggregate router validates sequence
    Given an aggregate at sequence 5
    When I receive a command at sequence 3
    Then the router should reject with sequence mismatch
    And no handler should be invoked

  Scenario: Aggregate router returns emitted events
    Given an aggregate router
    When a handler emits 2 events
    Then the router should return those events
    And the events should have correct sequences

  Scenario: Unknown command type returns error
    Given an aggregate router with handlers for "CreateOrder"
    When I receive an "UnknownCommand" command
    Then the router should return an error
    And the error should indicate unknown command type

  # ==========================================================================
  # Saga Router
  # ==========================================================================

  Scenario: Saga router dispatches by event type
    Given a saga router with handlers for "OrderCreated" and "OrderShipped"
    When I receive an "OrderCreated" event
    Then the OrderCreated handler should be invoked
    And the OrderShipped handler should NOT be invoked

  Scenario: Saga router fetches destination state
    Given a saga router
    When I receive an event that triggers command to "inventory"
    Then the router should fetch inventory aggregate state
    And the handler should receive destination state for sequence calculation

  Scenario: Saga router emits commands
    Given a saga router
    When a handler produces a command
    Then the router should return the command
    And the command should have correct saga_origin

  Scenario: Saga router handles rejection
    Given a saga router with a rejected command
    When the router processes the rejection
    Then the router should build compensation context
    And the router should emit rejection notification

  Scenario: Saga router is stateless
    Given a saga router
    When I process two events with same type
    Then each should be processed independently
    And no state should carry over between events

  # ==========================================================================
  # Projector Router
  # ==========================================================================

  Scenario: Projector router dispatches by event type
    Given a projector router with handlers for "OrderCreated"
    When I receive an "OrderCreated" event
    Then the OrderCreated handler should be invoked

  Scenario: Projector router processes event batches
    Given a projector router
    When I receive 5 events in a batch
    Then all 5 events should be processed in order
    And the router projection state should be returned

  Scenario: Projector router supports speculative execution
    Given a projector router
    When I speculatively process events
    Then no external side effects should occur
    And the projection result should be returned

  Scenario: Projector router tracks position
    Given a projector router
    When I process events from sequence 10 to 15
    Then the router should track that position 15 was processed

  # ==========================================================================
  # Process Manager Router
  # ==========================================================================

  Scenario: PM router dispatches by event type across domains
    Given a PM router with handlers for "OrderCreated" and "InventoryReserved"
    When I receive an "OrderCreated" event from domain "orders"
    Then the OrderCreated handler should be invoked
    When I receive an "InventoryReserved" event from domain "inventory"
    Then the InventoryReserved handler should be invoked

  Scenario: PM router requires correlation ID
    Given a PM router
    When I receive an event without correlation ID
    Then the event should be skipped
    And no handler should be invoked

  Scenario: PM router maintains state by correlation
    Given a PM router
    When I receive correlated events with ID "workflow-123"
    Then state should be maintained across events
    And events with different correlation IDs should have separate state

  Scenario: PM router emits commands
    Given a PM router
    When a handler produces a command
    Then the router should return the command
    And the command should preserve correlation ID

  # ==========================================================================
  # Handler Registration
  # ==========================================================================

  Scenario: Register handler by type URL suffix
    Given a router
    When I register handler for type "OrderCreated"
    Then events ending with "OrderCreated" should match
    And events ending with "ItemAdded" should NOT match

  Scenario: Register multiple handlers
    Given a router
    When I register handlers for "TypeA", "TypeB", and "TypeC"
    Then all three types should be routable
    And each should invoke its specific handler

  Scenario: Handler receives decoded message
    Given a router with handler for protobuf message type
    When I receive an event with that type
    Then the handler should receive the decoded message
    And the raw bytes should be deserialized

  # ==========================================================================
  # State Building
  # ==========================================================================

  Scenario: State is built from events
    Given an aggregate router
    And events: OrderCreated, ItemAdded, ItemAdded
    When I build state from these events
    Then the state should reflect all three events applied
    And the state should have 2 items

  Scenario: State building uses snapshot when available
    Given an aggregate router
    And a snapshot at sequence 5
    And events 6, 7, 8
    When I build state
    Then the router should start from snapshot
    And only apply events 6, 7, 8

  Scenario: Empty aggregate has default state
    Given an aggregate router
    And no events for the aggregate
    When I build state
    Then the state should be the default/initial state

  # ==========================================================================
  # Error Handling in Routers
  # ==========================================================================

  Scenario: Handler error is propagated
    Given a router
    When a handler returns an error
    Then the router should propagate the error
    And no events should be emitted

  Scenario: Deserialization error is handled
    Given a router
    When I receive an event with invalid payload
    Then the router should return an error
    And the error should indicate deserialization failure

  Scenario: State building error is handled
    Given a router
    When state building fails
    Then the router should return an error
    And no handler should be invoked

  # ==========================================================================
  # Guard/Validate/Compute Pattern
  # ==========================================================================

  Scenario: Guard checks preconditions
    Given an aggregate with guard checking aggregate exists
    When I send command to non-existent aggregate
    Then guard should reject
    And no event should be emitted

  Scenario: Validate checks command validity
    Given an aggregate handler with validation
    When I send command with invalid data
    Then validate should reject
    And rejection reason should describe the issue

  Scenario: Compute produces events
    Given an aggregate handler
    When guard and validate pass
    Then compute should produce events
    And events should reflect the state change
