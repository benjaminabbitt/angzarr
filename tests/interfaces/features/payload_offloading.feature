# docs:start:offloading_contract
Feature: Payload offloading for large events
  When event payloads exceed message bus size limits, they are stored externally
  and replaced with a PayloadReference marker (claim check pattern). This enables
  handling of arbitrarily large event data while keeping the event bus efficient.

  Key behaviors:
  - Payloads below threshold are stored inline (no offloading)
  - Payloads above threshold are stored externally via PayloadStore
  - Content-addressable storage (SHA-256) enables deduplication
  - Payload references can be resolved back to full content
  - Integrity verification detects corruption during retrieval
# docs:end:offloading_contract

  Background:
    Given a PayloadStore test environment

  # ==========================================================================
  # Threshold Behavior
  # ==========================================================================

  Scenario: Small payloads remain inline
    Given an offloading threshold of 1024 bytes
    When I store an event with a 100 byte payload
    Then the event should have an inline payload
    And the payload store should have 0 items

  Scenario: Large payloads are offloaded
    Given an offloading threshold of 100 bytes
    When I store an event with a 500 byte payload
    Then the event should have an external payload reference
    And the payload store should have 1 item

  Scenario: Boundary case - small payload at threshold stays inline
    Given an offloading threshold of 500 bytes
    When I store an event with a 100 byte payload
    Then the event should have an inline payload

  # ==========================================================================
  # Content-Addressable Storage
  # ==========================================================================

  Scenario: Identical payloads share storage
    Given an offloading threshold of 100 bytes
    When I store two events with identical 500 byte payloads
    Then the payload store should have 1 item
    And both references should have the same content hash

  Scenario: Different payloads have separate storage
    Given an offloading threshold of 100 bytes
    When I store two events with different 500 byte payloads
    Then the payload store should have 2 items
    And the references should have different content hashes

  Scenario: Content hash is SHA-256
    Given an offloading threshold of 100 bytes
    When I store an event with a known payload
    Then the reference content hash should be 32 bytes

  # ==========================================================================
  # Reference Structure
  # ==========================================================================

  Scenario: Reference includes storage type
    Given an offloading threshold of 100 bytes
    And the payload store uses filesystem storage
    When I store an event with a 500 byte payload
    Then the reference should have storage type FILESYSTEM

  Scenario: Reference includes URI
    Given an offloading threshold of 100 bytes
    When I store an event with a 500 byte payload
    Then the reference URI should be valid

  Scenario: Reference includes original size
    Given an offloading threshold of 100 bytes
    When I store an event with a 500 byte payload
    Then the reference should indicate original size of 500 bytes

  Scenario: Reference includes storage timestamp
    Given an offloading threshold of 100 bytes
    When I store an event with a 500 byte payload
    Then the reference should include a storage timestamp

  # ==========================================================================
  # Payload Retrieval
  # ==========================================================================

  Scenario: External payload can be retrieved
    Given an offloading threshold of 100 bytes
    And I have stored an event with a 500 byte payload
    When I resolve the payload reference
    Then I should get the original payload content

  Scenario: Retrieved payload matches original exactly
    Given an offloading threshold of 10 bytes
    And I store an event with large text payload
    When I resolve the payload reference
    Then the retrieved payload should match the original

  Scenario: Integrity verification passes for valid content
    Given an offloading threshold of 100 bytes
    And I have stored an event with a valid payload
    When I retrieve the payload
    Then the integrity check should pass

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Missing payload returns error
    Given a reference to a non-existent payload
    When I try to resolve the reference
    Then the operation should fail with NOT_FOUND

  Scenario: Corrupted payload fails integrity check
    Given an offloading threshold of 100 bytes
    And I have stored an event with a payload
    When the stored payload becomes corrupted
    And I try to resolve the reference
    Then the operation should fail with INTEGRITY_FAILED

  # ==========================================================================
  # TTL Cleanup
  # ==========================================================================

  Scenario: Old payloads can be cleaned up
    Given an offloading threshold of 100 bytes
    And I have stored payloads with various ages
    When I run TTL cleanup for payloads older than 1 hour
    Then old payloads should be deleted
    And recent payloads should be retained

  # ==========================================================================
  # Disabled Offloading
  # ==========================================================================

  Scenario: No threshold means no offloading
    Given offloading is disabled
    When I store an event with a 10000 byte payload
    Then the event should have an inline payload
