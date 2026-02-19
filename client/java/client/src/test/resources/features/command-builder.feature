Feature: CommandBuilder
  CommandBuilder provides a fluent API for constructing commands. Raw protobuf
  construction is verbose and error-prone. The builder pattern:

  - **Reduces boilerplate**: Chain method calls instead of nested object construction
  - **Enforces correctness**: Type-safe methods prevent invalid field combinations
  - **Provides defaults**: Auto-generates correlation IDs, handles optional fields
  - **Enables composition**: Build incrementally, execute when ready

  Fluent builders are idiomatic in all target languages (Java, C#, C++) and
  dramatically improve developer experience for polyglot teams.

  Background:
    Given an AggregateClient connected to the coordinator

  Scenario: Build command with explicit field values
    # The builder must accept all CommandBook fields: domain, root, correlation_id,
    # sequence, and the command payload. This is the foundational use case.
    When I build a command using CommandBuilder:
      | field          | value                                |
      | domain         | test                                 |
      | root           | 550e8400-e29b-41d4-a716-446655440000 |
      | correlation_id | corr-123                             |
      | sequence       | 5                                    |
    Then the resulting CommandBook should have:
      | field          | value                                |
      | domain         | test                                 |
      | root           | 550e8400-e29b-41d4-a716-446655440000 |
      | correlation_id | corr-123                             |
      | sequence       | 5                                    |

  Scenario: Auto-generate correlation ID for distributed tracing
    # Correlation IDs link related operations across services. When not provided,
    # the builder must generate a UUID to ensure every command can be traced.
    # Without this, debugging distributed flows becomes impossible.
    When I build a command without specifying correlation_id
    Then the resulting CommandBook should have a non-empty correlation_id

  Scenario: Build command for new aggregate (no root UUID)
    # Creating a new aggregate requires a command without a root UUID.
    # The aggregate coordinator will generate the root and return it in the response.
    # The builder must support this "create" use case distinctly from "update".
    When I build a command for domain "test" without specifying root
    Then the resulting CommandBook should have no root UUID

  Scenario: Build and execute in single fluent chain
    # The most common pattern: construct and send in one expression.
    # Separating build() from execute() is useful for testing or logging,
    # but the combined flow must be ergonomic for production code.
    Given a registered aggregate handler for domain "test"
    When I use CommandBuilder to build and execute a command
    Then I should receive a CommandResponse

  Scenario: Method chaining returns builder for fluent composition
    # Each builder method must return the builder instance (this/self).
    # This enables: client.command(domain, root).withCorrelationId(x).withSequence(y).execute()
    # Failing to return the builder breaks the fluent pattern.
    When I create a CommandBuilder for domain "test"
    And I chain with_correlation_id "chain-test"
    And I chain with_sequence 10
    And I chain with_command for a TestCommand message
    And I call build
    Then the CommandBook should reflect all chained values

  Scenario: Sequence defaults to zero for new commands
    # Sequence numbers enable optimistic concurrency control.
    # For new aggregates, sequence must be 0. The builder should default to 0
    # when not specified, as this is the most common case.
    When I build a command without specifying sequence
    Then the resulting CommandBook should have sequence 0

  Scenario: Payload serialization handles protobuf messages
    # Commands contain protobuf Any-wrapped payloads. The builder must:
    # 1. Accept typed protobuf messages
    # 2. Serialize to bytes
    # 3. Wrap in Any with correct type_url
    # This hides proto complexity from SDK users.
    When I build a command with_command "type.googleapis.com/test.CreateItem" and message
    Then the resulting CommandBook should have:
      | field    | value                               |
      | type_url | type.googleapis.com/test.CreateItem |
    And the payload should be correctly serialized
