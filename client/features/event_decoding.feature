Feature: Event Decoding - Payload Deserialization
  Events are stored as google.protobuf.Any with type_url and value.
  Decoding extracts typed messages from the Any wrapper based on
  type_url matching. This is fundamental for state building and projections.

  # ==========================================================================
  # Basic Decoding
  # ==========================================================================

  Scenario: Decode event with matching type URL
    Given an event with type_url "type.googleapis.com/orders.OrderCreated"
    And valid protobuf bytes for OrderCreated
    When I decode the event as OrderCreated
    Then decoding should succeed
    And I should get an OrderCreated message

  Scenario: Decode event with type URL suffix match
    Given an event with type_url "type.googleapis.com/orders.OrderCreated"
    When I decode looking for suffix "OrderCreated"
    Then decoding should succeed
    And the full type_url prefix should be ignored

  Scenario: Decode returns None for type mismatch
    Given an event with type_url "type.googleapis.com/orders.ItemAdded"
    When I decode the event as OrderCreated
    Then decoding should return None/null
    And no error should be raised

  # ==========================================================================
  # EventPage Structure
  # ==========================================================================

  Scenario: EventPage contains sequence
    Given an EventPage at sequence 5
    Then event.sequence should be 5

  Scenario: EventPage contains created_at timestamp
    Given an EventPage with timestamp
    Then event.created_at should be a valid timestamp
    And the timestamp should be parseable

  Scenario: EventPage payload is Event variant
    Given an EventPage with Event payload
    Then event.payload should be Event variant
    And the Event should contain the Any wrapper

  Scenario: EventPage payload can be PayloadReference
    Given an EventPage with offloaded payload
    Then event.payload should be PayloadReference variant
    And the reference should contain storage details

  # ==========================================================================
  # Type URL Handling
  # ==========================================================================

  Scenario: Full type URL matching
    Given an event with type_url "type.googleapis.com/myapp.events.v1.OrderCreated"
    When I match against "type.googleapis.com/myapp.events.v1.OrderCreated"
    Then the match should succeed

  Scenario: Suffix matching for convenience
    Given an event with type_url "type.googleapis.com/myapp.events.v1.OrderCreated"
    When I match against suffix "OrderCreated"
    Then the match should succeed

  Scenario: Suffix matching is case-sensitive
    Given an event with type_url ending in "OrderCreated"
    When I match against suffix "ordercreated"
    Then the match should fail

  Scenario: Versioned type URLs
    Given events with type_urls:
      | type.googleapis.com/myapp.events.v1.OrderCreated |
      | type.googleapis.com/myapp.events.v2.OrderCreated |
    When I match against "v1.OrderCreated"
    Then only the v1 event should match

  # ==========================================================================
  # Payload Bytes
  # ==========================================================================

  Scenario: Payload bytes are valid protobuf
    Given an event with properly encoded payload
    When I decode the payload bytes
    Then the protobuf message should deserialize correctly
    And all fields should be populated

  Scenario: Empty payload bytes
    Given an event with empty payload bytes
    When I decode the payload
    Then the message should have default values
    And no error should occur (empty protobuf is valid)

  Scenario: Corrupted payload bytes
    Given an event with corrupted payload bytes
    When I attempt to decode
    Then decoding should fail
    And an error should indicate deserialization failure

  # ==========================================================================
  # Nil/None Handling
  # ==========================================================================

  Scenario: EventPage with no payload
    Given an EventPage with payload = None
    When I attempt to decode
    Then decoding should return None/null
    And no crash should occur

  Scenario: Event with no value bytes
    Given an Event Any with empty value
    When I decode
    Then the result should be a default message
    And no error should occur

  # ==========================================================================
  # Helper Functions
  # ==========================================================================

  Scenario: decode_event helper function
    Given the decode_event<T>(event, type_suffix) function
    When I call decode_event(event, "OrderCreated")
    Then if type matches, Some(T) is returned
    And if type doesn't match, None is returned

  Scenario: events_from_response helper
    Given a CommandResponse with events
    When I call events_from_response(response)
    Then I should get a slice/list of EventPages

  Scenario: events_from_response with no events
    Given a CommandResponse with no events
    When I call events_from_response(response)
    Then I should get an empty slice/list

  # ==========================================================================
  # Batch Processing
  # ==========================================================================

  Scenario: Decode multiple events of same type
    Given 5 events all of type "ItemAdded"
    When I decode each as ItemAdded
    Then all 5 should decode successfully
    And each should have correct data

  Scenario: Decode mixed event types
    Given events: OrderCreated, ItemAdded, ItemAdded, OrderShipped
    When I decode by type
    Then OrderCreated should decode as OrderCreated
    And ItemAdded events should decode as ItemAdded
    And OrderShipped should decode as OrderShipped

  Scenario: Filter events by type
    Given events: OrderCreated, ItemAdded, ItemAdded, OrderShipped
    When I filter for "ItemAdded" events
    Then I should get 2 events
    And both should be ItemAdded type
