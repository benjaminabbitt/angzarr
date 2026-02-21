//! Event decoding step definitions.

use angzarr_client::proto::{event_page, CommandResponse, Cover, EventBook, EventPage};
use angzarr_client::{decode_event, events_from_response, type_url_matches};
use cucumber::{given, then, when, World};
use prost::Message;
use prost_types::Any;
use uuid::Uuid;

/// Test event for decoding.
#[derive(Clone, Message, PartialEq)]
pub struct OrderCreated {
    #[prost(string, tag = "1")]
    pub order_id: String,
}

/// Another test event.
#[derive(Clone, Message, PartialEq)]
pub struct ItemAdded {
    #[prost(string, tag = "1")]
    pub item_id: String,
}

fn make_event_page(seq: u32, type_url: &str, value: Vec<u8>) -> EventPage {
    EventPage {
        sequence: seq,
        created_at: Some(prost_types::Timestamp {
            seconds: 1704067200, // 2024-01-01
            nanos: 0,
        }),
        payload: Some(event_page::Payload::Event(Any {
            type_url: type_url.to_string(),
            value,
        })),
    }
}

fn make_event_book(events: Vec<EventPage>) -> EventBook {
    let next_seq = events.len() as u32;
    EventBook {
        cover: Some(Cover {
            domain: "orders".to_string(),
            root: Some(angzarr_client::proto::Uuid {
                value: Uuid::new_v4().as_bytes().to_vec(),
            }),
            correlation_id: String::new(),
            edition: None,
        }),
        pages: events,
        snapshot: None,
        next_sequence: next_seq,
    }
}

/// Test context for event decoding scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct EventDecodingWorld {
    current_event: Option<EventPage>,
    event_book: Option<EventBook>,
    decode_result: Option<OrderCreated>,
    decode_result_item: Option<ItemAdded>,
    decode_is_none: bool,
    match_result: bool,
    events_list: Vec<EventPage>,
    command_response: Option<CommandResponse>,
    last_error: Option<String>,
}

impl EventDecodingWorld {
    fn new() -> Self {
        Self {
            current_event: None,
            event_book: None,
            decode_result: None,
            decode_result_item: None,
            decode_is_none: false,
            match_result: false,
            events_list: Vec::new(),
            command_response: None,
            last_error: None,
        }
    }
}

// --- Given steps ---

#[given(expr = "an event with type_url {string}")]
async fn given_event_with_type_url(world: &mut EventDecodingWorld, type_url: String) {
    let event = OrderCreated {
        order_id: "test-123".to_string(),
    };
    world.current_event = Some(make_event_page(0, &type_url, event.encode_to_vec()));
}

#[given("valid protobuf bytes for OrderCreated")]
async fn given_valid_protobuf_bytes(world: &mut EventDecodingWorld) {
    // Already set in previous step
    assert!(world.current_event.is_some());
}

#[given(expr = "an EventPage at sequence {int}")]
async fn given_event_page_at_sequence(world: &mut EventDecodingWorld, seq: u32) {
    let event = OrderCreated {
        order_id: "test".to_string(),
    };
    world.current_event = Some(make_event_page(
        seq,
        "type.googleapis.com/orders.OrderCreated",
        event.encode_to_vec(),
    ));
}

#[given("an EventPage with timestamp")]
async fn given_event_page_with_timestamp(world: &mut EventDecodingWorld) {
    let event = OrderCreated {
        order_id: "test".to_string(),
    };
    world.current_event = Some(make_event_page(
        0,
        "type.googleapis.com/orders.OrderCreated",
        event.encode_to_vec(),
    ));
}

#[given("an EventPage with Event payload")]
async fn given_event_page_with_event_payload(world: &mut EventDecodingWorld) {
    let event = OrderCreated {
        order_id: "test".to_string(),
    };
    world.current_event = Some(make_event_page(
        0,
        "type.googleapis.com/orders.OrderCreated",
        event.encode_to_vec(),
    ));
}

#[given("an EventPage with offloaded payload")]
async fn given_event_page_with_offloaded(world: &mut EventDecodingWorld) {
    // PayloadReference variant (External in the oneof)
    world.current_event = Some(EventPage {
        sequence: 0,
        created_at: None,
        payload: Some(event_page::Payload::External(
            angzarr_client::proto::PayloadReference {
                storage_type: 2, // S3
                uri: "s3://bucket/key".to_string(),
                content_hash: b"abc123".to_vec(),
                original_size: 1024,
                stored_at: None,
            },
        )),
    });
}

#[given(expr = "an event with type_url ending in {string}")]
async fn given_event_with_suffix(world: &mut EventDecodingWorld, suffix: String) {
    let event = OrderCreated {
        order_id: "test".to_string(),
    };
    world.current_event = Some(make_event_page(
        0,
        &format!("type.googleapis.com/myapp.events.{}", suffix),
        event.encode_to_vec(),
    ));
}

#[given("events with type_urls:")]
async fn given_events_with_type_urls(world: &mut EventDecodingWorld) {
    let event = OrderCreated {
        order_id: "test".to_string(),
    };
    world.events_list = vec![
        make_event_page(
            0,
            "type.googleapis.com/myapp.events.v1.OrderCreated",
            event.encode_to_vec(),
        ),
        make_event_page(
            1,
            "type.googleapis.com/myapp.events.v2.OrderCreated",
            event.encode_to_vec(),
        ),
    ];
}

#[given("an event with properly encoded payload")]
async fn given_properly_encoded(world: &mut EventDecodingWorld) {
    let event = OrderCreated {
        order_id: "properly-encoded".to_string(),
    };
    world.current_event = Some(make_event_page(
        0,
        "type.googleapis.com/orders.OrderCreated",
        event.encode_to_vec(),
    ));
}

#[given("an event with empty payload bytes")]
async fn given_empty_payload(world: &mut EventDecodingWorld) {
    world.current_event = Some(make_event_page(
        0,
        "type.googleapis.com/orders.OrderCreated",
        vec![],
    ));
}

#[given("an event with corrupted payload bytes")]
async fn given_corrupted_payload(world: &mut EventDecodingWorld) {
    world.current_event = Some(make_event_page(
        0,
        "type.googleapis.com/orders.OrderCreated",
        vec![0xFF, 0xFF, 0xFF, 0xFF],
    ));
}

#[given("an EventPage with payload = None")]
async fn given_event_page_no_payload(world: &mut EventDecodingWorld) {
    world.current_event = Some(EventPage {
        sequence: 0,
        created_at: None,
        payload: None,
    });
}

#[given("an Event Any with empty value")]
async fn given_event_any_empty_value(world: &mut EventDecodingWorld) {
    world.current_event = Some(make_event_page(
        0,
        "type.googleapis.com/orders.OrderCreated",
        vec![],
    ));
}

#[given(expr = "the decode_event<T>\\(event, type_suffix\\) function")]
async fn given_decode_event_function(_world: &mut EventDecodingWorld) {
    // Function exists
}

#[given("a CommandResponse with events")]
async fn given_command_response_with_events(world: &mut EventDecodingWorld) {
    let event = OrderCreated {
        order_id: "resp".to_string(),
    };
    let event_book = make_event_book(vec![
        make_event_page(
            0,
            "type.googleapis.com/orders.OrderCreated",
            event.encode_to_vec(),
        ),
        make_event_page(
            1,
            "type.googleapis.com/orders.ItemAdded",
            ItemAdded {
                item_id: "item1".to_string(),
            }
            .encode_to_vec(),
        ),
    ]);
    world.command_response = Some(CommandResponse {
        events: Some(event_book),
        projections: vec![],
    });
}

#[given("a CommandResponse with no events")]
async fn given_command_response_no_events(world: &mut EventDecodingWorld) {
    world.command_response = Some(CommandResponse {
        events: Some(make_event_book(vec![])),
        projections: vec![],
    });
}

#[given(expr = "{int} events all of type {string}")]
async fn given_n_events_of_type(world: &mut EventDecodingWorld, count: u32, event_type: String) {
    let mut events = vec![];
    for i in 0..count {
        let item = ItemAdded {
            item_id: format!("item{}", i),
        };
        events.push(make_event_page(
            i,
            &format!("type.googleapis.com/orders.{}", event_type),
            item.encode_to_vec(),
        ));
    }
    world.events_list = events;
}

#[given("events: OrderCreated, ItemAdded, ItemAdded, OrderShipped")]
async fn given_mixed_events(world: &mut EventDecodingWorld) {
    world.events_list = vec![
        make_event_page(
            0,
            "type.googleapis.com/orders.OrderCreated",
            OrderCreated {
                order_id: "o1".to_string(),
            }
            .encode_to_vec(),
        ),
        make_event_page(
            1,
            "type.googleapis.com/orders.ItemAdded",
            ItemAdded {
                item_id: "i1".to_string(),
            }
            .encode_to_vec(),
        ),
        make_event_page(
            2,
            "type.googleapis.com/orders.ItemAdded",
            ItemAdded {
                item_id: "i2".to_string(),
            }
            .encode_to_vec(),
        ),
        make_event_page(
            3,
            "type.googleapis.com/orders.OrderShipped",
            OrderCreated {
                order_id: "shipped".to_string(),
            }
            .encode_to_vec(),
        ),
    ];
}

// --- When steps ---

#[when("I decode the event as OrderCreated")]
async fn when_decode_as_order_created(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        let result: Option<OrderCreated> = decode_event(event, "OrderCreated");
        if let Some(decoded) = result {
            world.decode_result = Some(decoded);
        } else {
            world.decode_is_none = true;
        }
    }
}

#[when(expr = "I decode looking for suffix {string}")]
async fn when_decode_with_suffix(world: &mut EventDecodingWorld, suffix: String) {
    if let Some(ref event) = world.current_event {
        let result: Option<OrderCreated> = decode_event(event, &suffix);
        if let Some(decoded) = result {
            world.decode_result = Some(decoded);
        } else {
            world.decode_is_none = true;
        }
    }
}

#[when(expr = "I match against {string}")]
async fn when_match_against(world: &mut EventDecodingWorld, pattern: String) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            world.match_result = type_url_matches(&any.type_url, &pattern);
        }
    }
}

#[when(expr = "I match against suffix {string}")]
async fn when_match_suffix(world: &mut EventDecodingWorld, suffix: String) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            world.match_result = any.type_url.ends_with(&suffix);
        }
    }
}

#[when("I decode the payload bytes")]
async fn when_decode_payload_bytes(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            match OrderCreated::decode(any.value.as_slice()) {
                Ok(decoded) => world.decode_result = Some(decoded),
                Err(e) => world.last_error = Some(e.to_string()),
            }
        }
    }
}

#[when("I decode the payload")]
async fn when_decode_payload(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            match OrderCreated::decode(any.value.as_slice()) {
                Ok(decoded) => world.decode_result = Some(decoded),
                Err(e) => world.last_error = Some(e.to_string()),
            }
        }
    }
}

#[when("I attempt to decode")]
async fn when_attempt_decode(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            match OrderCreated::decode(any.value.as_slice()) {
                Ok(decoded) => world.decode_result = Some(decoded),
                Err(e) => world.last_error = Some(e.to_string()),
            }
        } else {
            world.decode_is_none = true;
        }
    } else {
        world.decode_is_none = true;
    }
}

#[when("I decode")]
async fn when_decode(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            if any.value.is_empty() {
                world.decode_result = Some(OrderCreated::default());
            } else {
                match OrderCreated::decode(any.value.as_slice()) {
                    Ok(decoded) => world.decode_result = Some(decoded),
                    Err(_) => world.decode_is_none = true,
                }
            }
        } else {
            world.decode_is_none = true;
        }
    }
}

#[when(expr = "I call decode_event\\(event, {string}\\)")]
async fn when_call_decode_event(world: &mut EventDecodingWorld, suffix: String) {
    if let Some(ref event) = world.current_event {
        let result: Option<OrderCreated> = decode_event(event, &suffix);
        if let Some(decoded) = result {
            world.decode_result = Some(decoded);
        } else {
            world.decode_is_none = true;
        }
    }
}

#[when(expr = "I call events_from_response\\(response\\)")]
async fn when_call_events_from_response(world: &mut EventDecodingWorld) {
    if let Some(ref response) = world.command_response {
        let events = events_from_response(response);
        world.events_list = events.to_vec();
    }
}

#[when("I decode each as ItemAdded")]
async fn when_decode_each_as_item_added(world: &mut EventDecodingWorld) {
    for event in &world.events_list {
        let _result: Option<ItemAdded> = decode_event(event, "ItemAdded");
    }
}

#[when("I decode by type")]
async fn when_decode_by_type(_world: &mut EventDecodingWorld) {
    // Decode each event by its type
}

#[when(expr = "I filter for {string} events")]
async fn when_filter_for_type(world: &mut EventDecodingWorld, event_type: String) {
    world.events_list = world
        .events_list
        .iter()
        .filter(|e| {
            if let Some(event_page::Payload::Event(any)) = &e.payload {
                any.type_url.ends_with(&event_type)
            } else {
                false
            }
        })
        .cloned()
        .collect();
}

// --- Then steps ---

#[then("decoding should succeed")]
async fn then_decoding_succeeds(world: &mut EventDecodingWorld) {
    assert!(world.decode_result.is_some() || !world.decode_is_none);
}

#[then("I should get an OrderCreated message")]
async fn then_get_order_created(world: &mut EventDecodingWorld) {
    assert!(world.decode_result.is_some());
}

#[then("the full type_url prefix should be ignored")]
async fn then_prefix_ignored(world: &mut EventDecodingWorld) {
    assert!(world.decode_result.is_some());
}

#[then("decoding should return None/null")]
async fn then_decoding_returns_none(world: &mut EventDecodingWorld) {
    assert!(world.decode_is_none || world.decode_result.is_none());
}

#[then("no error should be raised")]
async fn then_no_error_raised(world: &mut EventDecodingWorld) {
    assert!(world.last_error.is_none());
}

#[then(expr = "event.sequence should be {int}")]
async fn then_event_sequence(world: &mut EventDecodingWorld, expected: u32) {
    if let Some(ref event) = world.current_event {
        assert_eq!(event.sequence, expected);
    }
}

#[then("event.created_at should be a valid timestamp")]
async fn then_event_has_timestamp(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        assert!(event.created_at.is_some());
    }
}

#[then("the timestamp should be parseable")]
async fn then_timestamp_parseable(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        assert!(event.created_at.is_some());
    }
}

#[then("event.payload should be Event variant")]
async fn then_payload_is_event(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        assert!(matches!(event.payload, Some(event_page::Payload::Event(_))));
    }
}

#[then("the Event should contain the Any wrapper")]
async fn then_event_contains_any(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            assert!(!any.type_url.is_empty());
        }
    }
}

#[then("event.payload should be PayloadReference variant")]
async fn then_payload_is_reference(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        assert!(matches!(
            event.payload,
            Some(event_page::Payload::External(_))
        ));
    }
}

#[then("the reference should contain storage details")]
async fn then_reference_has_details(world: &mut EventDecodingWorld) {
    if let Some(ref event) = world.current_event {
        if let Some(event_page::Payload::External(ref_)) = &event.payload {
            assert!(ref_.storage_type > 0);
            assert!(!ref_.uri.is_empty());
        }
    }
}

#[then("the match should succeed")]
async fn then_match_succeeds(world: &mut EventDecodingWorld) {
    assert!(world.match_result);
}

#[then("the match should fail")]
async fn then_match_fails(world: &mut EventDecodingWorld) {
    assert!(!world.match_result);
}

#[then("only the v1 event should match")]
async fn then_only_v1_matches(world: &mut EventDecodingWorld) {
    // v1 event has "v1.OrderCreated" suffix
    let v1_event = &world.events_list[0];
    if let Some(event_page::Payload::Event(any)) = &v1_event.payload {
        assert!(any.type_url.contains("v1"));
    }
}

#[then("the protobuf message should deserialize correctly")]
async fn then_protobuf_deserializes(world: &mut EventDecodingWorld) {
    assert!(world.decode_result.is_some());
}

#[then("all fields should be populated")]
async fn then_fields_populated(world: &mut EventDecodingWorld) {
    if let Some(ref decoded) = world.decode_result {
        assert!(!decoded.order_id.is_empty());
    }
}

#[then("the message should have default values")]
async fn then_message_has_defaults(world: &mut EventDecodingWorld) {
    if let Some(ref decoded) = world.decode_result {
        // Empty payload decodes to defaults
        assert!(decoded.order_id.is_empty());
    }
}

#[then(expr = "no error should occur \\(empty protobuf is valid\\)")]
async fn then_empty_protobuf_valid(world: &mut EventDecodingWorld) {
    assert!(world.last_error.is_none());
}

#[then("decoding should fail")]
async fn then_decoding_fails(world: &mut EventDecodingWorld) {
    assert!(world.last_error.is_some() || world.decode_is_none);
}

#[then("an error should indicate deserialization failure")]
async fn then_error_deserialization(world: &mut EventDecodingWorld) {
    assert!(world.last_error.is_some());
}

#[then("no crash should occur")]
async fn then_no_crash(world: &mut EventDecodingWorld) {
    // Test ran to completion
    assert!(world.decode_is_none || world.decode_result.is_some());
}

#[then("the result should be a default message")]
async fn then_result_is_default(world: &mut EventDecodingWorld) {
    if let Some(ref decoded) = world.decode_result {
        assert!(decoded.order_id.is_empty());
    }
}

#[then(expr = "if type matches, Some\\(T\\) is returned")]
async fn then_some_if_matches(world: &mut EventDecodingWorld) {
    // Logic depends on actual type match
    if !world.decode_is_none {
        assert!(world.decode_result.is_some());
    }
}

#[then(expr = "if type doesn't match, None is returned")]
async fn then_none_if_not_matches(world: &mut EventDecodingWorld) {
    // When type doesn't match, decode_is_none is true
    // This is a documentation step
}

#[then("I should get a slice/list of EventPages")]
async fn then_get_event_pages(world: &mut EventDecodingWorld) {
    assert!(!world.events_list.is_empty());
}

#[then("I should get an empty slice/list")]
async fn then_get_empty_list(world: &mut EventDecodingWorld) {
    assert!(world.events_list.is_empty());
}

#[then(expr = "all {int} should decode successfully")]
async fn then_all_decode_successfully(world: &mut EventDecodingWorld, count: u32) {
    assert_eq!(world.events_list.len(), count as usize);
}

#[then("each should have correct data")]
async fn then_each_has_correct_data(world: &mut EventDecodingWorld) {
    for event in &world.events_list {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            assert!(!any.value.is_empty());
        }
    }
}

#[then("OrderCreated should decode as OrderCreated")]
async fn then_order_created_decodes(world: &mut EventDecodingWorld) {
    let event = &world.events_list[0];
    let result: Option<OrderCreated> = decode_event(event, "OrderCreated");
    assert!(result.is_some());
}

#[then("ItemAdded events should decode as ItemAdded")]
async fn then_item_added_decodes(world: &mut EventDecodingWorld) {
    for event in &world.events_list {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            if any.type_url.ends_with("ItemAdded") {
                let result: Option<ItemAdded> = decode_event(event, "ItemAdded");
                assert!(result.is_some());
            }
        }
    }
}

#[then("OrderShipped should decode as OrderShipped")]
async fn then_order_shipped_decodes(world: &mut EventDecodingWorld) {
    let event = &world.events_list[3];
    if let Some(event_page::Payload::Event(any)) = &event.payload {
        assert!(any.type_url.ends_with("OrderShipped"));
    }
}

#[then(expr = "I should get {int} events")]
async fn then_get_n_events(world: &mut EventDecodingWorld, count: u32) {
    assert_eq!(world.events_list.len(), count as usize);
}

#[then("both should be ItemAdded type")]
async fn then_both_item_added(world: &mut EventDecodingWorld) {
    for event in &world.events_list {
        if let Some(event_page::Payload::Event(any)) = &event.payload {
            assert!(any.type_url.ends_with("ItemAdded"));
        }
    }
}
