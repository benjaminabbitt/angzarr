Feature: Aggregate command handler
  The aggregate command handler orchestrates command execution through
  a context factory. It supports both synchronous gRPC calls and async
  bus delivery, with sync projector integration.

  Without proper command handling, aggregates cannot process business
  logic or persist events. This contract ensures the handler correctly
  delegates to the context factory and integrates with projectors.

  Background:
    Given an aggregate handler test environment

  # ==========================================================================
  # Domain Identity
  # ==========================================================================

  Scenario: Handler reports correct domain
    Given an aggregate handler for domain "player"
    Then the handler domain should be "player"

  Scenario: Handler domain is not empty
    Given an aggregate handler for domain "order"
    Then the handler domain should not be empty

  Scenario: Different handlers report different domains
    Given aggregate handlers for domains "order", "inventory", "fulfillment", "customer"
    Then each handler should report its configured domain

  # ==========================================================================
  # Command Execution
  # ==========================================================================

  Scenario: Execute returns events from client logic
    Given an aggregate handler that produces events
    When I execute a command
    Then the response should contain events
    And the response should have at least one event page

  Scenario: Execute includes events from client logic
    Given an aggregate handler that produces a "PlayerCreated" event
    When I execute a command
    Then the response should contain events
    And the events should include the produced event

  # ==========================================================================
  # Sync Projector Integration
  # ==========================================================================

  Scenario: Sync projector called when events present
    Given an aggregate handler with a tracking projector
    And the handler produces events
    When I execute a command
    Then the sync projector should have been called

  Scenario: Sync projector skipped when no events
    Given an aggregate handler with a tracking projector
    And the handler produces no events
    When I execute a command
    Then the sync projector should not have been called

  Scenario: Sync projector output included in response
    Given an aggregate handler with a sync projector
    And the handler produces events
    When I execute a command
    Then the response should include projector output

  # ==========================================================================
  # Command Bus Transport
  # ==========================================================================

  Scenario: Wrap command for bus preserves cover
    Given a command for domain "player" with correlation "test-correlation"
    When the command is wrapped for bus transport
    Then the wrapped event book should have domain "player"
    And the wrapped event book should have correlation "test-correlation"

  Scenario: Wrap command creates single page
    Given a command book
    When the command is wrapped for bus transport
    Then the wrapped event book should have exactly 1 page

  Scenario: Wrap command sets correct type URL
    Given a command book
    When the command is wrapped for bus transport
    Then the wrapped page type URL should end with "angzarr.CommandBook"

  # ==========================================================================
  # Command Extraction
  # ==========================================================================

  Scenario: Extract command roundtrips correctly
    Given a command for domain "player"
    When the command is wrapped and then extracted
    Then the extracted command should have domain "player"

  Scenario: Extract returns none for empty pages
    Given an event book with no pages
    When I try to extract a command
    Then extraction should return none

  Scenario: Extract returns none for non-command type URL
    Given an event book with type URL "type.googleapis.com/some.OtherType"
    When I try to extract a command
    Then extraction should return none

  Scenario: Extract returns none for missing payload
    Given an event book with missing payload
    When I try to extract a command
    Then extraction should return none
