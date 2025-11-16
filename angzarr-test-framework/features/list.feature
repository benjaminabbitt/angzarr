Feature: Kernel Linked List
  As a kernel developer
  I want to use intrusive doubly-linked lists
  So that I can efficiently manage kernel data structures

  Scenario: Creating an empty list
    Given an empty list
    Then the list should not be empty after initialization

  Scenario: Adding entries to a list
    Given an empty list
    When I add an entry to the list
    Then the list should not be empty

  Scenario: Multiple entries maintain order
    Given an empty list
    When I add an entry to the list
    And I add an entry to the list
    And I add an entry to the list
    Then the list should contain 3 entries
    And the entries should be properly linked
