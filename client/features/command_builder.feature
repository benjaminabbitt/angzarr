# docs:start:command_builder_contract
Feature: CommandBuilder - Fluent Command Construction
  The CommandBuilder provides a fluent API for constructing commands.
  It handles serialization, correlation IDs, sequence numbers, and
  type URLs while providing compile-time and runtime validation.

  The builder pattern enables both OO-style client usage and can be
  adapted for router-based implementations.
# docs:end:command_builder_contract

  Background:
    Given a mock GatewayClient for testing

  # ==========================================================================
  # Basic Command Construction
  # ==========================================================================

  Scenario: Build command with all required fields
    When I build a command for domain "orders" root "order-001"
      And I set the command type to "CreateOrder"
      And I set the command payload
    Then the built command should have domain "orders"
    And the built command should have root "order-001"
    And the built command should have type URL containing "CreateOrder"

  Scenario: Build command for new aggregate (no root)
    When I build a command for new aggregate in domain "orders"
      And I set the command type to "CreateOrder"
      And I set the command payload
    Then the built command should have domain "orders"
    And the built command should have no root

  Scenario: Build generates correlation ID when not provided
    When I build a command for domain "orders"
      And I set the command type and payload
    Then the built command should have a non-empty correlation ID
    And the correlation ID should be a valid UUID

  # ==========================================================================
  # Optional Fields
  # ==========================================================================

  Scenario: Build with explicit correlation ID
    When I build a command for domain "orders"
      And I set correlation ID to "trace-123"
      And I set the command type and payload
    Then the built command should have correlation ID "trace-123"

  Scenario: Build with sequence number
    When I build a command for domain "orders" root "order-002"
      And I set sequence to 5
      And I set the command type and payload
    Then the built command should have sequence 5

  Scenario: Build without sequence defaults to 0
    When I build a command for domain "orders"
      And I set the command type and payload
    Then the built command should have sequence 0

  # ==========================================================================
  # Validation
  # ==========================================================================

  Scenario: Build without command type fails
    When I build a command for domain "orders"
      And I do NOT set the command type
    Then building should fail
    And the error should indicate missing type URL

  Scenario: Build without payload fails
    When I build a command for domain "orders"
      And I set the command type to "CreateOrder"
      And I do NOT set the payload
    Then building should fail
    And the error should indicate missing payload

  # ==========================================================================
  # Fluent Chaining
  # ==========================================================================

  Scenario: Builder methods can be chained
    When I build a command using fluent chaining:
      """
      client.command("orders", root)
        .with_correlation_id("trace-456")
        .with_sequence(3)
        .with_command("CreateOrder", payload)
        .build()
      """
    Then the build should succeed
    And all chained values should be preserved

  Scenario: Builder is immutable-friendly
    Given a builder configured for domain "orders"
    When I create two commands with different roots
    Then each command should have its own root
    And builder reuse should not cause cross-contamination

  # ==========================================================================
  # Execute Integration
  # ==========================================================================

  Scenario: Builder can execute directly
    When I build and execute a command for domain "orders"
    Then the command should be sent to the gateway
    And the response should be returned

  Scenario: Execute without building explicitly
    When I use the builder to execute directly:
      """
      client.command("orders", root)
        .with_command("CreateOrder", payload)
        .execute()
      """
    Then the command should be built and executed in one call

  # ==========================================================================
  # Merge Strategy
  # ==========================================================================

  Scenario: Default merge strategy is COMMUTATIVE
    When I build a command without specifying merge strategy
    Then the command page should have MERGE_COMMUTATIVE strategy

  Scenario: Build with explicit merge strategy
    When I build a command with merge strategy STRICT
    Then the command page should have MERGE_STRICT strategy

  # ==========================================================================
  # Extension Traits
  # ==========================================================================

  Scenario: Client provides command builder shortcut
    Given a GatewayClient implementation
    When I call client.command("orders", root)
    Then I should receive a CommandBuilder for that domain and root

  Scenario: Client provides command_new shortcut
    Given a GatewayClient implementation
    When I call client.command_new("orders")
    Then I should receive a CommandBuilder with no root set
