Feature: Client Library Parity
  Tests core client library patterns that must be consistent across all language implementations.
  These tests ensure Rust, Go, and Python client libraries provide equivalent functionality.

  # --- CommandRouter Pattern ---

  Scenario: CommandRouter registers handlers by type suffix
    Given a CommandRouter for "test" domain
    And a state rebuilder that returns empty state
    When I register a handler for "TestCommand"
    Then the router has 1 registered command type
    And the router reports "TestCommand" in its command types

  Scenario: CommandRouter dispatches to matching handler
    Given a CommandRouter for "player" domain
    And a handler for "RegisterPlayer" that emits "PlayerRegistered"
    And a ContextualCommand with:
      | domain | type_url                                    | root     |
      | player | type.googleapis.com/examples.RegisterPlayer | player-1 |
    When the router dispatches the command
    Then the handler is invoked
    And the response contains a "PlayerRegistered" event

  Scenario: CommandRouter rejects unknown command types
    Given a CommandRouter for "player" domain
    And a handler for "RegisterPlayer" only
    And a ContextualCommand with type "UnknownCommand"
    When the router dispatches the command
    Then an unimplemented error is returned

  Scenario: CommandRouter rebuilds state before dispatch
    Given a CommandRouter for "player" domain
    And a state rebuilder that counts events
    And an EventBook with 3 events
    And a handler that reads state
    When the router dispatches a command
    Then the state rebuilder was called with 3 events
    And the handler received the rebuilt state

  # --- EventRouter Pattern ---

  Scenario: EventRouter registers handlers by type suffix
    Given an EventRouter for "saga-test" subscribing to "source" domain
    And sends to "target" domain with "TargetCommand" command
    When I register a handler for "SourceEvent"
    Then the router has 1 registered event type
    And the router reports "SourceEvent" in its event types

  Scenario: EventRouter dispatches to matching handler
    Given an EventRouter for "saga-table-hand" subscribing to "table" domain
    And sends to "hand" domain with "DealCards" command
    And a handler for "HandStarted" that emits "DealCards"
    And an EventBook with a HandStarted event
    When the router dispatches the event
    Then the handler is invoked
    And the response contains a CommandBook for "hand" domain

  Scenario: EventRouter returns empty for no matching handler
    Given an EventRouter for "saga-test" subscribing to "source" domain
    And sends to "other" domain with "OtherCommand" command
    And a handler for "EventA" only
    And an EventBook with an "EventB" event
    When the router dispatches the event
    Then no command is returned
    And no error is raised

  Scenario: EventRouter prepare returns destinations
    Given an EventRouter with a prepare handler for "HandStarted"
    And the prepare handler returns a Cover for "hand" domain
    And an EventBook with a HandStarted event
    When the router prepares destinations
    Then the destinations include "hand" domain
    And the destination root matches the event hand_root

  Scenario: EventRouter multi-handler emits multiple commands
    Given an EventRouter with a multi-handler for "PotAwarded"
    And a PotAwarded event with 2 winners
    When the router dispatches the event
    Then 2 CommandBooks are returned
    And each command targets a different player root

  # --- ComponentDescriptor Auto-Derivation ---

  Scenario: CommandRouter descriptor includes registered types
    Given a CommandRouter for "player" domain
    And handlers for "RegisterPlayer" and "DepositFunds"
    When I get the component descriptor
    Then the descriptor has component_type "aggregate"
    And the descriptor outputs include "player" domain
    And the output types include "RegisterPlayer" and "DepositFunds"

  Scenario: EventRouter descriptor includes input and output
    Given an EventRouter for "saga-table-hand" subscribing to "table" domain
    And sends to "hand" domain with "DealCards" command
    And a handler for "HandStarted"
    When I get the component descriptor
    Then the descriptor has component_type "saga"
    And the descriptor inputs include "table" domain
    And the input types include "HandStarted"
    And the descriptor outputs include "hand" domain
    And the output types include "DealCards"

  # --- Handler Wrapper Pattern ---

  Scenario: AggregateHandler wraps CommandRouter
    Given an AggregateHandler with a CommandRouter
    When I call GetDescriptor
    Then the descriptor is returned from the router
    When I call Handle with a ContextualCommand
    Then the request is forwarded to the router

  Scenario: SagaHandler wraps EventRouter
    Given a SagaHandler with an EventRouter
    When I call GetDescriptor
    Then the descriptor is returned from the router
    When I call Prepare with an EventBook
    Then destinations are returned from the router prepare
    When I call Execute with source and destinations
    Then commands are returned from the router dispatch

  Scenario: ProjectorHandler invokes handle function
    Given a ProjectorHandler for "test" projector
    And domains "player" and "table"
    And a handle function that returns sequence 5
    When I call Handle with an EventBook
    Then the handle function is invoked
    And the response has projector "test"
    And the response has sequence 5

  # --- Server Runner Pattern ---

  Scenario: Server runner configures from environment
    Given environment variable UDS_BASE_PATH="/tmp/test"
    And environment variable SERVICE_NAME="business"
    When I create a server config
    Then the transport type is "uds"
    And the socket path includes "/tmp/test"

  Scenario: Server runner falls back to TCP
    Given no UDS_BASE_PATH environment variable
    And port 50001
    When I create a server config
    Then the transport type is "tcp"
    And the address is "0.0.0.0:50001"

  # --- Proto Helper Pattern ---

  Scenario: EventBook next_sequence returns page count
    Given an EventBook with 5 event pages
    Then next_sequence returns 5

  Scenario: EventBook next_sequence handles empty book
    Given an empty EventBook
    Then next_sequence returns 0

  Scenario: Type URL construction is consistent
    Given type name "RegisterPlayer"
    When I construct a type URL
    Then the result is "type.googleapis.com/examples.RegisterPlayer"

  Scenario: Type URL suffix matching works
    Given type URL "type.googleapis.com/examples.RegisterPlayer"
    Then type URL ends with "RegisterPlayer"
    And type URL ends with "examples.RegisterPlayer"
    And type URL does not end with "DepositFunds"

  # --- Error Handling Pattern ---

  Scenario: CommandRejectedError has reason
    Given a CommandRejectedError with reason "Insufficient funds"
    Then the error message contains "Insufficient funds"
    And the error converts to FAILED_PRECONDITION status

  Scenario: Handler can reject commands
    Given a handler that rejects with "Player not found"
    When the router dispatches a command
    Then the response is an error
    And the error reason is "Player not found"

  # --- Sequence Validation Pattern ---

  Scenario: Command sequence from destination state
    Given a destination EventBook with next_sequence 7
    When I create a CommandBook targeting that destination
    Then the command page sequence is 7

  Scenario: Event sequence uses next sequence
    Given a current sequence of 3
    When I create an EventPage for sequence 3
    Then the event page has sequence 3

  # --- Process Manager Pattern ---

  Scenario: ProcessManagerRouter registers handlers
    Given a ProcessManagerRouter for "pm-test" with domain "pm-domain"
    And subscriptions to "domain-a" and "domain-b"
    And sends to "target" domain with "TargetCommand" command
    When I register a handler for "TriggerEvent"
    Then the router has 1 registered event type
    And the router reports input domains "domain-a" and "domain-b"

  Scenario: ProcessManagerRouter rebuilds state from process events
    Given a ProcessManagerRouter with a state rebuilder
    And a trigger EventBook from "domain-a"
    And a process state EventBook with 4 events
    When the router dispatches the trigger
    Then the state rebuilder was called with the process state
    And the handler received the rebuilt PM state

  Scenario: ProcessManagerRouter prepare returns destinations
    Given a ProcessManagerRouter with a prepare handler
    And a trigger EventBook with correlation_id "corr-123"
    And a process state EventBook
    When the router prepares destinations
    Then the destinations are returned from the prepare handler
    And the PM state was rebuilt for the prepare call

  Scenario: ProcessManagerHandler wraps ProcessManagerRouter
    Given a ProcessManagerHandler with a router
    When I call GetDescriptor
    Then the descriptor has component_type "process_manager"
    When I call Prepare with trigger and process_state
    Then destinations are returned
    When I call Handle with trigger, process_state, and destinations
    Then commands and process_events are returned
