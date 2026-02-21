# docs:start:bus_contract
Feature: EventBus interface
  The EventBus distributes committed events to interested subscribers. After
  an aggregate persists events, the bus broadcasts them to sagas, projectors,
  and process managers that need to react.

  Why decoupling matters:
  - Aggregates don't know who's listening -- add projectors without changing aggregates
  - Subscribers don't know event sources -- multiple aggregates can trigger the same saga
  - Independent deployment -- update a projector without redeploying the aggregate

  Delivery guarantee: at-least-once
  - Network failures, process restarts -- events may be redelivered
  - Handlers MUST be idempotent (use PositionStore to detect duplicates)
  - Why not exactly-once? It requires distributed transactions, adds latency,
    and handlers need idempotency anyway for replay scenarios

  What breaks if this contract is violated:
  - Missing events -- projectors have stale read models, sagas miss triggers
  - Corrupted payloads -- business data is wrong, state reconstruction fails
  - Broken filtering -- handlers receive irrelevant events, waste resources

  The backend is selected via the BUS_BACKEND environment variable.
  Supported backends: channel, ipc, amqp, kafka, sns_sqs, pubsub

  Examples use the poker domain (player, table, hand) because poker mechanics
  directly exercise event bus patterns:
  - Fan-out: HandComplete triggers hand-player-saga (transfer winnings),
    hand-table-saga (signal table), AND output-projector (update display)
  - Domain filtering: player-projector subscribes to "player" domain only;
    doesn't waste cycles processing thousands of ActionTaken events from hands
  - Compensation notification: when JoinTable is rejected (table full), the
    Notification routes back through the bus to trigger FundsReleased

  Patterns enabled by this interface:
  - Saga activation: hand events trigger the hand-player-saga to update balances
  - Projector updates: all domain events flow to the output-projector for display
  - Compensation notification: when commands fail, rejections flow back through
    the bus to notify source aggregates
# docs:end:bus_contract

  Background:
    Given an EventBus backend

  # ==========================================================================
  # Basic Publish/Subscribe
  # ==========================================================================
  # The core operation: aggregates publish after persisting, handlers subscribe
  # to react. This is how the system becomes reactive rather than procedural.

  # docs:start:bus_pubsub
  Scenario: Events flow from aggregates to handlers
    # A player aggregate commits PlayerRegistered to the EventStore.
    # The player-projector, subscribed to "player", receives it to update read models.
    Given the player aggregate publishes events to the bus
    And the player-projector subscribes to the player domain
    When the player-projector starts listening
    And a PlayerRegistered event is published
    Then the player-projector receives the event
    And can update its read model accordingly

  Scenario: Publishing succeeds even without subscribers
    # Day 1: deploy the player aggregate. No projectors yet.
    # Events must still persist and publish successfully.
    # Later, projectors will catch up via event replay.
    Given the player aggregate is deployed with no subscribers
    When it publishes a PlayerRegistered event
    Then the publish succeeds (events are persisted regardless)
    # Subscribers added later will catch up from the EventStore

  Scenario: Batched events all reach the subscriber
    # A StartHand command might emit: HandStarted, CardsDealt, BlindPosted
    # All three must reach the subscriber - no silent drops.
    Given an aggregate that emits multiple events per command
    And a subscriber listening for those events
    When the aggregate publishes 5 events in a batch
    Then the subscriber receives all 5 events

  Scenario: Events arrive in sequence order from a single publisher
    # Handlers often depend on processing events in order.
    # CardsDealt must arrive before ActionTaken for correct state.
    Given a single-threaded hand aggregate publishing events
    And a projector subscribed to hand
    When events with sequences 0, 1, 2, 3, 4 are published in order
    Then the projector receives them in sequence order: 0, 1, 2, 3, 4
  # docs:end:bus_pubsub

  # ==========================================================================
  # Domain Filtering
  # ==========================================================================
  # Handlers declare interest by domain. They receive only relevant events.
  # This keeps handlers focused and prevents wasted processing.

  Scenario: Handlers only receive events from their subscribed domain
    # The player-projector cares about PlayerRegistered, FundsDeposited, etc.
    # It does NOT care about HandStarted or TableCreated.
    # Without filtering, every handler would process every event - wasteful.
    Given the player-projector subscribes only to the player domain
    When events are published to player and table domains
    Then the player-projector receives only player events
    And never sees table events (filtered out by the bus)

  Scenario: Cross-domain handlers can subscribe to multiple domains
    # The output-projector displays game state across player, table, and hand.
    # Its projector needs events from all three domains.
    Given the output-projector subscribed to player and table domains
    When events are published to player, table, and hand domains
    Then the output-projector receives player events (subscribed)
    And the output-projector receives table events (subscribed)
    And the output-projector does NOT receive hand events (not subscribed)

  # ==========================================================================
  # Fan-out to Multiple Subscribers
  # ==========================================================================
  # One event may trigger multiple reactions. The bus copies events to all
  # interested subscribers - they don't compete for a single copy.

  # docs:start:bus_fanout
  Scenario: Multiple handlers independently process the same event
    # HandComplete triggers:
    # - output-projector: updates the game display
    # - hand-player-saga: transfers winnings to player balances
    # - hand-table-saga: signals the table to end the hand
    # Each handler must receive the event independently.
    Given three handlers subscribe to the hand domain:
      | output-projector   |
      | hand-player-saga   |
      | hand-table-saga    |
    When a HandComplete event is published
    Then all three handlers receive the event
    And each processes it independently (no competition for the message)
  # docs:end:bus_fanout

  # ==========================================================================
  # Event Data Integrity
  # ==========================================================================
  # Events carry business data that must survive serialization and transport.
  # Corruption would cause handlers to operate on wrong data.

  Scenario: Routing metadata survives transport
    # event_type: handlers use this to select which code path to execute
    # correlation_id: process managers use this to correlate related events
    # Both must arrive exactly as published.
    Given a projector listening for HandComplete events
    When a HandComplete event is published with correlation_id "hand-flow-123"
    Then the projector receives event_type "HandComplete" (for routing)
    And correlation_id "hand-flow-123" (for process correlation)

  Scenario: Payload bytes are preserved exactly through transport
    # Event payloads are protobuf-serialized business data.
    # Even one bit flip would corrupt deserialization.
    Given a handler expecting protobuf-encoded event data
    When an event is published with payload bytes [1, 2, 3, 4, 5]
    Then the handler receives exactly [1, 2, 3, 4, 5]
    # No encoding conversion, compression artifacts, or truncation

  # ==========================================================================
  # Error Handling
  # ==========================================================================
  # Handler failures must surface, not silently disappear. This enables
  # retry logic, dead-letter queues, alerting, and debugging.

  Scenario: Handler failures are visible to the system
    # The analytics-projector throws an exception processing OrderCreated.
    # This failure must be reported - not swallowed - so we can:
    # - Retry the event (transient failure)
    # - Send to dead-letter queue (poison message)
    # - Alert on-call (systematic failure)
    Given a handler that will fail when processing events
    When an event is delivered to that handler
    Then the handler's error is reported (not swallowed)
    # The bus/framework can now decide: retry, DLQ, or escalate

  # ==========================================================================
  # Concurrent Publishing
  # ==========================================================================
  # Under load, many aggregates publish simultaneously. The bus must not
  # lose events due to contention or race conditions.

  Scenario: High concurrency doesn't cause event loss
    # 10 hand aggregates process commands simultaneously.
    # Each publishes an event at roughly the same time.
    # All 10 events must reach subscribers - no silent drops.
    Given multiple hand aggregates processing commands in parallel
    And a projector subscribed to hand
    When 10 events are published concurrently (racing)
    Then the projector eventually receives all 10 events
    # Order may vary, but count must be exact
