# docs:start:sync_modes_contract
Feature: Projector synchronization modes
  Projectors can be configured as synchronous or asynchronous, controlling
  whether command responses wait for projector processing and include
  projector output.

  Two modes exist:
  - Async (default): Fire and forget. Command returns immediately after
    event persistence. Projectors run in the background.
  - Sync: Wait for projectors. Command blocks until sync projectors complete
    and their output is included in the response.

  Without sync projectors, clients would have no way to get immediate read-after-
  write consistency for projections. Sync mode enables this for projector output
  while keeping other projectors async for performance.
# docs:end:sync_modes_contract

  Background:
    Given a SyncMode test environment

  # ==========================================================================
  # Async Mode (Default)
  # ==========================================================================

  Scenario: Async projectors run in background
    Given an async projector is registered
    When I execute a command without specifying sync mode
    Then the command should succeed
    And the response should not include projections
    And the async projector should eventually receive the events

  # ==========================================================================
  # Sync Mode
  # ==========================================================================

  Scenario: Sync projectors block command response
    Given a sync projector is registered
    When I execute a command with SIMPLE sync mode
    Then the command should succeed
    And the response should include the projector output
    And the projector should have processed before response

  Scenario: Sync projector output in response
    Given a sync projector named "analytics" is registered
    When I execute a command with SIMPLE sync mode
    Then the response should include a projection from "analytics"
    And the projection should have the correct sequence

  Scenario: Multiple sync projectors
    Given sync projectors "analytics" and "reporting" are registered
    When I execute a command with SIMPLE sync mode
    Then the response should include projections from both projectors

  Scenario: Mixed sync and async projectors
    Given a sync projector "sync-proj" is registered
    And an async projector "async-proj" is registered
    When I execute a command with SIMPLE sync mode
    Then the response should include projection from "sync-proj"
    And the async projector should eventually receive the events

  # ==========================================================================
  # Event Delivery
  # ==========================================================================

  Scenario: Sync projector receives complete event book
    Given a sync projector is registered
    When I execute a command that produces 3 events with SIMPLE sync mode
    Then the projector should receive all 3 events in one book

  Scenario: Projector receives domain and root
    Given a sync projector is registered
    When I execute a command for domain "orders" with SIMPLE sync mode
    Then the projector should receive events with domain "orders"
    And the events should have the correct aggregate root

  # ==========================================================================
  # Edge Cases
  # ==========================================================================

  Scenario: No projectors registered
    Given no projectors are registered
    When I execute a command with SIMPLE sync mode
    Then the command should succeed
    And the response should have empty projections

  Scenario: Failing projector does not fail command
    Given a failing sync projector is registered
    When I execute a command with SIMPLE sync mode
    Then the command should succeed
    And the events should still be persisted

