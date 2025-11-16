Feature: Memory Management
  As a kernel developer
  I want safe memory allocation
  So that I can manage kernel memory without leaks

  Scenario: Allocating kernel memory
    Given the memory allocator is initialized
    When I allocate 1024 bytes with GFP_KERNEL flags
    Then the allocation should succeed
    And the memory should be usable

  Scenario: Freeing kernel memory
    Given the memory allocator is initialized
    And I have allocated memory
    When I free the memory
    Then the memory should be returned to the allocator
    And future allocations should succeed

  Scenario: Handling allocation failures
    Given the memory allocator is under pressure
    When I try to allocate memory with GFP_NOWAIT
    Then the allocation may fail gracefully
    And no memory should be leaked
