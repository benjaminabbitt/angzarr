# docs:start:editions_contract
Feature: Edition timeline isolation
  Editions provide isolated timelines for event streams. Each edition maintains
  its own sequence numbers and event history, separate from the main timeline.

  Use cases for editions:
  - Speculative execution: Try commands without affecting the main timeline
  - A/B testing: Run different versions of business logic in parallel
  - Temporal branching: Explore "what-if" scenarios from historical points
  - Migration: Test schema changes before applying to main timeline

  Key behaviors:
  - Main timeline uses edition name "angzarr" (or empty string)
  - Named editions are completely isolated from the main timeline
  - Sequences restart from 0 in each edition for each aggregate
  - Edition events can be deleted without affecting the main timeline
# docs:end:editions_contract

  Background:
    Given an Edition test environment

  # ==========================================================================
  # Main Timeline
  # ==========================================================================

  Scenario: Main timeline uses default edition
    When I add an event to domain "order" on main timeline
    Then the event should be stored with edition "angzarr"
    And I should be able to retrieve the event from main timeline

  Scenario: Empty edition name maps to main timeline
    When I add an event to domain "order" with edition ""
    Then I should be able to retrieve the event with edition "angzarr"

  # ==========================================================================
  # Edition Isolation
  # ==========================================================================

  Scenario: Events in named edition are isolated from main timeline
    Given an aggregate "order" on main timeline with 3 events
    When I add 2 events to the same aggregate in edition "v2"
    Then main timeline should have 3 events
    And edition "v2" should have 2 events

  Scenario: Named editions are isolated from each other
    Given an aggregate "order" with root "order-001"
    When I add 2 events in edition "alpha"
    And I add 3 events in edition "beta"
    Then edition "alpha" should have 2 events for root "order-001"
    And edition "beta" should have 3 events for root "order-001"

  Scenario: Same aggregate can exist in multiple editions
    Given an aggregate "order" with root "shared-root"
    When I add 1 event on main timeline
    And I add 1 event in edition "test-edition"
    Then main timeline should have 1 event for root "shared-root"
    And edition "test-edition" should have 1 event for root "shared-root"

  # ==========================================================================
  # Sequence Isolation
  # ==========================================================================

  Scenario: Sequences are independent per edition
    Given an aggregate "order" on main timeline with 5 events
    When I add an event to the same aggregate in edition "branch"
    Then the first event in edition "branch" should have sequence 0
    And the next sequence on main timeline should be 5

  Scenario: Edition sequences start from zero
    When I add 3 events to aggregate "order" in edition "fresh"
    Then the events should have sequences 0, 1, 2
    And the next sequence in edition "fresh" should be 3

  # ==========================================================================
  # Root Discovery
  # ==========================================================================

  Scenario: List roots returns only roots in specified edition
    Given an aggregate "order" with root "main-only" on main timeline
    And an aggregate "order" with root "edition-only" in edition "separate"
    When I list roots for domain "order" on main timeline
    Then I should see 1 root in the list
    And root "main-only" should be in the list

  Scenario: Roots in multiple editions are listed per edition
    Given an aggregate "order" with root "multi-edition" on main timeline
    And an aggregate "order" with root "multi-edition" in edition "branched"
    When I list roots for domain "order" on main timeline
    Then I should see 1 root in the list
    When I list roots for domain "order" in edition "branched"
    Then I should see 1 root in the list

  # ==========================================================================
  # Edition Cleanup
  # ==========================================================================

  Scenario: Delete edition events removes all edition data
    Given an aggregate "order" on main timeline with 3 events
    And an aggregate "order" in edition "temp" with 5 events
    When I delete events for edition "temp" in domain "order"
    Then edition "temp" should have 0 events
    And main timeline should still have 3 events

  Scenario: Deleting edition does not affect other editions
    Given an aggregate "order" in edition "keep" with 2 events
    And an aggregate "order" in edition "remove" with 4 events
    When I delete events for edition "remove" in domain "order"
    Then edition "keep" should have 2 events
    And edition "remove" should have 0 events

  Scenario: Main timeline cannot be deleted
    Given an aggregate "order" on main timeline with 3 events
    When I try to delete events for edition "angzarr" in domain "order"
    Then the operation should be rejected

