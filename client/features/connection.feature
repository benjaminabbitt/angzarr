Feature: Connection - Client Connection Management
  Clients connect to angzarr services via TCP or Unix Domain Sockets.
  Connection configuration supports environment variables, explicit
  endpoints, and channel reuse for efficiency.

  # ==========================================================================
  # TCP Connection
  # ==========================================================================

  Scenario: Connect via TCP with host and port
    When I connect to "localhost:1310"
    Then the connection should succeed
    And the client should be ready for operations

  Scenario: Connect via TCP with http scheme
    When I connect to "http://localhost:1310"
    Then the connection should succeed
    And the scheme should be treated as insecure

  Scenario: Connect via TCP with https scheme
    When I connect to "https://localhost:1310"
    Then the connection should use TLS

  Scenario: Connect to non-existent host fails
    When I connect to "nonexistent.invalid:1310"
    Then the connection should fail
    And the error should indicate DNS or connection failure

  Scenario: Connect to closed port fails
    When I connect to "localhost:59999"
    Then the connection should fail
    And the error should indicate connection refused

  # ==========================================================================
  # Unix Domain Socket Connection
  # ==========================================================================

  Scenario: Connect via Unix socket path
    Given a Unix socket at "/tmp/angzarr.sock"
    When I connect to "/tmp/angzarr.sock"
    Then the connection should succeed
    And the client should use UDS transport

  Scenario: Connect via Unix socket with scheme
    Given a Unix socket at "/tmp/angzarr.sock"
    When I connect to "unix:///tmp/angzarr.sock"
    Then the connection should succeed

  Scenario: Connect to non-existent socket fails
    When I connect to "/tmp/nonexistent.sock"
    Then the connection should fail
    And the error should indicate socket not found

  # ==========================================================================
  # Environment Variable Configuration
  # ==========================================================================

  Scenario: Connect from environment variable
    Given environment variable "ANGZARR_ENDPOINT" set to "localhost:1310"
    When I call from_env("ANGZARR_ENDPOINT", "default:9999")
    Then the connection should use "localhost:1310"

  Scenario: Environment variable not set uses default
    Given environment variable "ANGZARR_ENDPOINT" is not set
    When I call from_env("ANGZARR_ENDPOINT", "localhost:1310")
    Then the connection should use "localhost:1310"

  Scenario: Empty environment variable uses default
    Given environment variable "ANGZARR_ENDPOINT" set to ""
    When I call from_env("ANGZARR_ENDPOINT", "localhost:1310")
    Then the connection should use "localhost:1310"

  # ==========================================================================
  # Channel Reuse
  # ==========================================================================

  Scenario: Create client from existing channel
    Given an existing gRPC channel
    When I call from_channel(channel)
    Then the client should reuse that channel
    And no new connection should be created

  Scenario: Multiple clients share channel
    Given an existing gRPC channel
    When I create QueryClient from the channel
    And I create AggregateClient from the same channel
    Then both clients should share the connection
    And the connection should only be established once

  # ==========================================================================
  # Client Types
  # ==========================================================================

  Scenario: QueryClient connects successfully
    When I create a QueryClient connected to "localhost:1310"
    Then the client should be able to query events

  Scenario: AggregateClient connects successfully
    When I create an AggregateClient connected to "localhost:1310"
    Then the client should be able to execute commands

  Scenario: SpeculativeClient connects successfully
    When I create a SpeculativeClient connected to "localhost:1310"
    Then the client should be able to perform speculative operations

  Scenario: DomainClient combines query and aggregate
    When I create a DomainClient connected to "localhost:1310"
    Then the client should have aggregate and query sub-clients
    And both should share the same connection

  Scenario: Client combines all operations
    When I create a Client connected to "localhost:1310"
    Then the client should have aggregate, query, and speculative sub-clients

  # ==========================================================================
  # Connection Options
  # ==========================================================================

  Scenario: Connection with timeout
    When I connect with timeout of 5 seconds
    Then the connection should respect the timeout
    And slow connections should fail after timeout

  Scenario: Connection with keep-alive
    When I connect with keep-alive enabled
    Then the connection should send keep-alive probes
    And idle connections should remain open

  # ==========================================================================
  # Error Handling
  # ==========================================================================

  Scenario: Invalid endpoint format
    When I connect to "not a valid endpoint"
    Then the connection should fail
    And the error should indicate invalid format

  Scenario: Connection lost mid-operation
    Given an established connection
    When the server disconnects
    And I attempt an operation
    Then the operation should fail
    And the error should indicate connection lost

  Scenario: Reconnection after failure
    Given a connection that failed
    When I create a new client with the same endpoint
    Then the new connection should be independent
    And the new connection should succeed if server is available
