# docs:start:upcasting_contract
Feature: Event upcasting for schema evolution
  Upcasting transforms old event versions to current versions during event loading.
  This enables schema evolution without data migration - old events are transformed
  on-the-fly when read.

  Use cases:
  - Schema evolution: Add/remove/rename fields across versions
  - Type migration: Change event type names while preserving behavior
  - Data enrichment: Add default values for new required fields

  Key behaviors:
  - Upcaster is invoked when loading events, before aggregate builds state
  - Event sequence numbers are preserved after transformation
  - Current version events pass through unchanged
  - Upcaster failure propagates as command failure
# docs:end:upcasting_contract

  Background:
    Given an Upcaster test environment

  # ==========================================================================
  # Schema Evolution
  # ==========================================================================

  Scenario: Old event versions are transformed to current version
    Given an upcaster that transforms V1 events to V2
    And events with type "OrderCreatedV1" are stored
    When I load the events through the upcaster
    Then the events should have type "OrderCreatedV2"
    And the events should have the migration marker

  Scenario: Current version events pass through unchanged
    Given an upcaster that transforms V1 events to V2
    And events with type "OrderCreatedV2" are stored
    When I load the events through the upcaster
    Then the events should still have type "OrderCreatedV2"

  Scenario: Mixed version events are transformed correctly
    Given an upcaster that transforms V1 events to V2
    And mixed version events are stored
    When I load the events through the upcaster
    Then the mixed events should all be V2

  # ==========================================================================
  # Sequence Preservation
  # ==========================================================================

  Scenario: Sequence numbers are preserved after upcasting
    Given an upcaster that transforms V1 events to V2
    And events with sequences 5, 6, 7 are stored
    When I load the events through the upcaster
    Then the events should have sequences 5, 6, 7

  Scenario: Event ordering is preserved
    Given an upcaster that transforms V1 events to V2
    And 10 sequential events are stored
    When I load the events through the upcaster
    Then the events should be in sequence order

  # ==========================================================================
  # Disabled Upcaster
  # ==========================================================================

  Scenario: Disabled upcaster passes events through unchanged
    Given upcasting is disabled
    And events with type "OrderCreatedV1" are stored
    When I load the events
    Then the events should still have type "OrderCreatedV1"

  Scenario: Empty events short-circuit without calling upcaster
    Given an upcaster that tracks invocations
    And no events are stored
    When I load the events through the upcaster
    Then the upcaster should not be invoked

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Upcaster failure propagates as error
    Given an upcaster that fails with "Schema migration failed"
    And events are stored
    When I try to load the events through the upcaster
    Then the operation should fail with "Schema migration failed"

  Scenario: Unknown event types pass through
    Given an upcaster that only handles known types
    And events with type "UnknownEventType" are stored
    When I load the events through the upcaster
    Then the events should pass through unchanged

  # ==========================================================================
  # Batch Processing
  # ==========================================================================

  Scenario: Large batches are processed correctly
    Given an upcaster that transforms V1 events to V2
    And 100 events are stored
    When I load the events through the upcaster
    Then all 100 events should be transformed

  Scenario: Domain is passed to upcaster
    Given an upcaster that tracks domains
    And events in domain "inventory" are stored
    When I load the events for domain "inventory"
    Then the upcaster should receive domain "inventory"
