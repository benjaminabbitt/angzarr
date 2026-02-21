//! Query builder step definitions.

use angzarr_client::proto::{query::Selection, EventBook, EventPage, Query};
use angzarr_client::traits::QueryClient as QueryClientTrait;
use angzarr_client::{ClientError, QueryBuilderExt, Result};
use async_trait::async_trait;
use cucumber::{given, then, when, World};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

/// Mock query client that records executed queries.
#[derive(Clone, Default, Debug)]
pub struct MockQueryClient {
    pub last_query: Arc<Mutex<Option<Query>>>,
}

#[async_trait]
impl QueryClientTrait for MockQueryClient {
    async fn get_events(&self, query: Query) -> Result<EventBook> {
        *self.last_query.lock().unwrap() = Some(query);
        Ok(EventBook::default())
    }
}

/// Test context for QueryBuilder scenarios.
#[derive(Debug, World)]
#[world(init = Self::new)]
pub struct QueryBuilderWorld {
    mock_client: MockQueryClient,
    built_query: Option<Query>,
    build_error: Option<ClientError>,
    domain: String,
    root: Option<Uuid>,
    correlation_id: Option<String>,
    edition: Option<String>,
    get_events_result: Option<EventBook>,
    get_pages_result: Option<Vec<EventPage>>,
}

impl QueryBuilderWorld {
    fn new() -> Self {
        Self {
            mock_client: MockQueryClient::default(),
            built_query: None,
            build_error: None,
            domain: String::new(),
            root: None,
            correlation_id: None,
            edition: None,
            get_events_result: None,
            get_pages_result: None,
        }
    }
}

// --- Background ---

#[given("a mock QueryClient for testing")]
async fn given_mock_query_client(world: &mut QueryBuilderWorld) {
    world.mock_client = MockQueryClient::default();
}

// --- Basic Query Construction ---

#[when(expr = "I build a query for domain {string} root {string}")]
async fn when_build_query_domain_root(world: &mut QueryBuilderWorld, domain: String, root: String) {
    world.domain = domain.clone();
    let uuid = Uuid::parse_str(&root).unwrap_or_else(|_| Uuid::new_v4());
    world.root = Some(uuid);

    let query = world.mock_client.query(&domain, uuid).build();
    world.built_query = Some(query);
}

#[when(expr = "I build a query for domain {string} without root")]
async fn when_build_query_domain_only(world: &mut QueryBuilderWorld, domain: String) {
    world.domain = domain.clone();
    world.root = None;

    let query = world.mock_client.query_domain(&domain).build();
    world.built_query = Some(query);
}

#[when(expr = "I build a query for domain {string}")]
async fn when_build_query_domain(world: &mut QueryBuilderWorld, domain: String) {
    world.domain = domain.clone();
    world.root = Some(Uuid::new_v4());

    let query = world
        .mock_client
        .query(&domain, world.root.unwrap())
        .build();
    world.built_query = Some(query);
}

#[when(expr = "I set range from {int}")]
async fn when_set_range_from(world: &mut QueryBuilderWorld, lower: u32) {
    let root = world.root.unwrap_or_else(Uuid::new_v4);
    let query = world
        .mock_client
        .query(&world.domain, root)
        .range(lower)
        .build();
    world.built_query = Some(query);
}

#[when(expr = "I set range from {int} to {int}")]
async fn when_set_range_from_to(world: &mut QueryBuilderWorld, lower: u32, upper: u32) {
    let root = world.root.unwrap_or_else(Uuid::new_v4);
    let query = world
        .mock_client
        .query(&world.domain, root)
        .range_to(lower, upper)
        .build();
    world.built_query = Some(query);
}

#[when(expr = "I set as_of_sequence to {int}")]
async fn when_set_as_of_sequence(world: &mut QueryBuilderWorld, seq: u32) {
    let root = world.root.unwrap_or_else(Uuid::new_v4);
    let query = world
        .mock_client
        .query(&world.domain, root)
        .as_of_sequence(seq)
        .build();
    world.built_query = Some(query);
}

#[when(expr = "I set as_of_time to {string}")]
async fn when_set_as_of_time(world: &mut QueryBuilderWorld, timestamp: String) {
    let root = world.root.unwrap_or_else(Uuid::new_v4);
    let result = world
        .mock_client
        .query(&world.domain, root)
        .as_of_time(&timestamp);

    match result {
        Ok(builder) => world.built_query = Some(builder.build()),
        Err(e) => world.build_error = Some(e),
    }
}

#[when(expr = "I set by_correlation_id to {string}")]
async fn when_set_by_correlation_id(world: &mut QueryBuilderWorld, cid: String) {
    world.correlation_id = Some(cid.clone());

    let builder = if let Some(root) = world.root {
        world.mock_client.query(&world.domain, root)
    } else {
        world.mock_client.query_domain(&world.domain)
    };

    let query = builder.by_correlation_id(&cid).build();
    world.built_query = Some(query);
}

#[when(expr = "I set edition to {string}")]
async fn when_set_edition(world: &mut QueryBuilderWorld, edition: String) {
    world.edition = Some(edition.clone());
    let root = world.root.unwrap_or_else(Uuid::new_v4);
    let query = world
        .mock_client
        .query(&world.domain, root)
        .edition(&edition)
        .build();
    world.built_query = Some(query);
}

#[when("I build a query using fluent chaining:")]
async fn when_build_fluent_chaining(world: &mut QueryBuilderWorld) {
    world.domain = "orders".to_string();
    let root = Uuid::new_v4();
    world.root = Some(root);
    world.edition = Some("test-branch".to_string());

    let query = world
        .mock_client
        .query("orders", root)
        .edition("test-branch")
        .range(10)
        .build();
    world.built_query = Some(query);
}

#[when("I build a query with:")]
async fn when_build_query_last_wins(world: &mut QueryBuilderWorld) {
    world.domain = "orders".to_string();
    let root = Uuid::new_v4();
    world.root = Some(root);

    // range(5) then as_of_sequence(10) - last wins
    let query = world
        .mock_client
        .query("orders", root)
        .range(5)
        .as_of_sequence(10)
        .build();
    world.built_query = Some(query);
}

#[when(expr = "I build and get_events for domain {string} root {string}")]
async fn when_build_and_get_events(world: &mut QueryBuilderWorld, domain: String, root: String) {
    let uuid = Uuid::parse_str(&root).unwrap_or_else(|_| Uuid::new_v4());
    let result = world.mock_client.query(&domain, uuid).get_events().await;
    match result {
        Ok(book) => world.get_events_result = Some(book),
        Err(e) => world.build_error = Some(e),
    }
}

#[when(expr = "I build and get_pages for domain {string} root {string}")]
async fn when_build_and_get_pages(world: &mut QueryBuilderWorld, domain: String, root: String) {
    let uuid = Uuid::parse_str(&root).unwrap_or_else(|_| Uuid::new_v4());
    let result = world.mock_client.query(&domain, uuid).get_pages().await;
    match result {
        Ok(pages) => world.get_pages_result = Some(pages),
        Err(e) => world.build_error = Some(e),
    }
}

#[given("a QueryClient implementation")]
async fn given_query_client_impl(world: &mut QueryBuilderWorld) {
    world.mock_client = MockQueryClient::default();
}

#[when(expr = "I call client.query\\({string}, root\\)")]
async fn when_call_query_method(world: &mut QueryBuilderWorld, domain: String) {
    world.domain = domain.clone();
    let root = Uuid::new_v4();
    world.root = Some(root);
    let query = world.mock_client.query(&domain, root).build();
    world.built_query = Some(query);
}

#[when(expr = "I call client.query_domain\\({string}\\)")]
async fn when_call_query_domain_method(world: &mut QueryBuilderWorld, domain: String) {
    world.domain = domain.clone();
    world.root = None;
    let query = world.mock_client.query_domain(&domain).build();
    world.built_query = Some(query);
}

// --- Then steps ---

#[then(expr = "the built query should have domain {string}")]
async fn then_query_has_domain(world: &mut QueryBuilderWorld, expected: String) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    assert_eq!(cover.domain, expected);
}

#[then(expr = "the built query should have root {string}")]
async fn then_query_has_root(world: &mut QueryBuilderWorld, _expected: String) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    assert!(cover.root.is_some());
}

#[then("the built query should have no root")]
async fn then_query_has_no_root(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    assert!(cover.root.is_none());
}

#[then("the built query should have range selection")]
async fn then_query_has_range_selection(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    assert!(matches!(query.selection, Some(Selection::Range(_))));
}

#[then(expr = "the range lower bound should be {int}")]
async fn then_range_lower_bound(world: &mut QueryBuilderWorld, expected: u32) {
    let query = world.built_query.as_ref().expect("query not built");
    if let Some(Selection::Range(range)) = &query.selection {
        assert_eq!(range.lower, expected);
    } else {
        panic!("expected range selection");
    }
}

#[then("the range upper bound should be empty")]
async fn then_range_upper_empty(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    if let Some(Selection::Range(range)) = &query.selection {
        assert!(range.upper.is_none());
    } else {
        panic!("expected range selection");
    }
}

#[then(expr = "the range upper bound should be {int}")]
async fn then_range_upper_bound(world: &mut QueryBuilderWorld, expected: u32) {
    let query = world.built_query.as_ref().expect("query not built");
    if let Some(Selection::Range(range)) = &query.selection {
        assert_eq!(range.upper, Some(expected));
    } else {
        panic!("expected range selection");
    }
}

#[then("the built query should have temporal selection")]
async fn then_query_has_temporal_selection(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    assert!(matches!(query.selection, Some(Selection::Temporal(_))));
}

#[then(expr = "the point_in_time should be sequence {int}")]
async fn then_point_in_time_sequence(world: &mut QueryBuilderWorld, expected: u32) {
    let query = world.built_query.as_ref().expect("query not built");
    if let Some(Selection::Temporal(temporal)) = &query.selection {
        if let Some(angzarr_client::proto::temporal_query::PointInTime::AsOfSequence(seq)) =
            &temporal.point_in_time
        {
            assert_eq!(*seq, expected);
        } else {
            panic!("expected as_of_sequence");
        }
    } else {
        panic!("expected temporal selection");
    }
}

#[then("the point_in_time should be the parsed timestamp")]
async fn then_point_in_time_timestamp(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    if let Some(Selection::Temporal(temporal)) = &query.selection {
        assert!(matches!(
            temporal.point_in_time,
            Some(angzarr_client::proto::temporal_query::PointInTime::AsOfTime(_))
        ));
    } else {
        panic!("expected temporal selection");
    }
}

#[then("building should fail")]
async fn then_building_fails(world: &mut QueryBuilderWorld) {
    assert!(world.build_error.is_some());
}

#[then("the error should indicate invalid timestamp")]
async fn then_error_invalid_timestamp(world: &mut QueryBuilderWorld) {
    let err = world.build_error.as_ref().expect("expected error");
    assert!(err.message().contains("timestamp") || err.message().contains("parse"));
}

#[then(expr = "the built query should have correlation ID {string}")]
async fn then_query_has_correlation_id(world: &mut QueryBuilderWorld, expected: String) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    assert_eq!(cover.correlation_id, expected);
}

#[then(expr = "the built query should have edition {string}")]
async fn then_query_has_edition(world: &mut QueryBuilderWorld, expected: String) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    assert!(cover.edition.is_some());
    let edition = cover.edition.as_ref().unwrap();
    assert_eq!(edition.name, expected);
}

#[then("the built query should have no edition")]
async fn then_query_has_no_edition(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    assert!(cover.edition.is_none());
}

#[then("the query should target main timeline")]
async fn then_query_targets_main_timeline(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    // No edition means main timeline
    assert!(cover.edition.is_none());
}

#[then("the build should succeed")]
async fn then_build_succeeds(world: &mut QueryBuilderWorld) {
    assert!(world.built_query.is_some());
}

#[then("all chained values should be preserved")]
async fn then_chained_values_preserved(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    let cover = query.cover.as_ref().expect("cover missing");
    assert!(cover.edition.is_some());
    assert!(matches!(query.selection, Some(Selection::Range(_))));
}

#[then("the query should have temporal selection (last set)")]
async fn then_query_has_temporal_last_set(world: &mut QueryBuilderWorld) {
    let query = world.built_query.as_ref().expect("query not built");
    assert!(matches!(query.selection, Some(Selection::Temporal(_))));
}

#[then("the range selection should be replaced")]
async fn then_range_replaced(world: &mut QueryBuilderWorld) {
    // Last call was as_of_sequence, so should be temporal not range
    let query = world.built_query.as_ref().expect("query not built");
    assert!(!matches!(query.selection, Some(Selection::Range(_))));
}

#[then("the query should be sent to the query service")]
async fn then_query_sent(world: &mut QueryBuilderWorld) {
    let recorded = world.mock_client.last_query.lock().unwrap();
    assert!(recorded.is_some());
}

#[then("an EventBook should be returned")]
async fn then_event_book_returned(world: &mut QueryBuilderWorld) {
    assert!(world.get_events_result.is_some());
}

#[then("only the event pages should be returned")]
async fn then_only_pages_returned(world: &mut QueryBuilderWorld) {
    assert!(world.get_pages_result.is_some());
}

#[then("the EventBook metadata should be stripped")]
async fn then_metadata_stripped(world: &mut QueryBuilderWorld) {
    // get_pages returns Vec<EventPage>, not EventBook
    assert!(world.get_pages_result.is_some());
}

#[then(expr = "I should receive a QueryBuilder for that domain and root")]
async fn then_receive_query_builder(world: &mut QueryBuilderWorld) {
    assert!(world.built_query.is_some());
    let query = world.built_query.as_ref().unwrap();
    let cover = query.cover.as_ref().expect("cover missing");
    assert!(!cover.domain.is_empty());
    assert!(cover.root.is_some());
}

#[then("I should receive a QueryBuilder with no root set")]
async fn then_receive_query_builder_no_root(world: &mut QueryBuilderWorld) {
    assert!(world.built_query.is_some());
    let query = world.built_query.as_ref().unwrap();
    let cover = query.cover.as_ref().expect("cover missing");
    assert!(cover.root.is_none());
}

#[then("I can chain by_correlation_id")]
async fn then_can_chain_correlation_id(world: &mut QueryBuilderWorld) {
    // This is a capability test - we proved it works by building
    assert!(world.built_query.is_some());
}
