Feature: EventBus interface
  The EventBus distributes committed events to interested subscribers. After
  an aggregate persists events, the bus broadcasts them to sagas, projectors,
  and process managers that need to react.

  Background:
    Given an EventBus backend

  Scenario: Events flow from aggregates to handlers
    Given the player aggregate publishes events to the bus
    And the player-projector subscribes to the player domain
    When the player-projector starts listening
    And a PlayerRegistered event is published
    Then the player-projector receives the event
    And can update its read model accordingly

  Scenario: Publishing succeeds even without subscribers
    Given the player aggregate is deployed with no subscribers
    When it publishes a PlayerRegistered event
    Then the publish succeeds even without subscribers

  Scenario: Batched events all reach the subscriber
    Given an aggregate that emits multiple events per command
    And a subscriber listening for those events
    When the aggregate publishes 5 events in a batch
    Then the subscriber receives all 5 events

  Scenario: Events arrive in sequence order from a single publisher
    Given a single-threaded hand aggregate publishing events
    And a projector subscribed to hand
    When events with sequences 0, 1, 2, 3, 4 are published in order
    Then the projector receives them in sequence order: 0, 1, 2, 3, 4

  Scenario: Handlers only receive events from their subscribed domain
    Given the player-projector subscribes only to the player domain
    When events are published to player and table domains
    Then the player-projector receives only player events
    And never sees table events which are filtered out by the bus

  Scenario: Cross-domain handlers can subscribe to multiple domains
    Given the output-projector subscribed to player and table domains
    When events are published to player, table, and hand domains
    Then the output-projector receives player events because it subscribed
    And the output-projector receives table events because it subscribed
    And the output-projector does NOT receive hand events because it did not subscribe

  Scenario: Multiple handlers independently process the same event
    Given three handlers subscribe to the hand domain:
      | handler_name     |
      | output-projector |
      | hand-player-saga |
      | hand-table-saga  |
    When a HandComplete event is published
    Then all three handlers receive the event
    And each processes it independently without competing for the message

  Scenario: Routing metadata survives transport
    Given a projector listening for HandComplete events
    When a HandComplete event is published with correlation_id "hand-flow-123"
    Then the projector receives event_type "HandComplete" for routing
    And the projector receives correlation_id "hand-flow-123" for process correlation

  Scenario: Payload bytes are preserved exactly through transport
    Given a handler expecting protobuf-encoded event data
    When an event is published with payload bytes [1, 2, 3, 4, 5]
    Then the handler receives exactly [1, 2, 3, 4, 5]

  Scenario: Handler failures are visible to the system
    Given a handler that will fail when processing events
    When an event is delivered to that handler
    Then the handler's error is reported and not swallowed

  Scenario: High concurrency doesn't cause event loss
    Given multiple hand aggregates processing commands in parallel
    And a projector subscribed to hand
    When 10 events are published concurrently and racing
    Then the projector eventually receives all 10 events
