Feature: Synchronization Primitives
  As a kernel developer
  I want safe synchronization primitives
  So that I can protect shared data structures

  Scenario: Spinlock protects data
    Given a spinlock protecting an integer
    When I acquire the lock
    And I modify the data
    And I release the lock
    Then the data should be modified correctly

  Scenario: Spinlock prevents concurrent access
    Given a spinlock protecting shared data
    When multiple threads try to acquire the lock
    Then only one thread should hold the lock at a time
    And all modifications should be visible after release
