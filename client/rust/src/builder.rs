//! Fluent builders for commands and queries.

use crate::convert::{parse_timestamp, uuid_to_proto};
use crate::error::{ClientError, Result};
use crate::proto::{
    query::Selection, temporal_query::PointInTime, CommandBook, CommandPage, CommandResponse,
    Cover, Edition, EventBook, EventPage, Query, SequenceRange, TemporalQuery,
};
use crate::traits;
use prost::Message;
use uuid::Uuid;

/// Builder for constructing and executing commands.
pub struct CommandBuilder<'a, C: traits::GatewayClient> {
    client: &'a C,
    domain: String,
    root: Option<Uuid>,
    correlation_id: Option<String>,
    sequence: u32,
    type_url: Option<String>,
    payload: Option<Vec<u8>>,
}

impl<'a, C: traits::GatewayClient> CommandBuilder<'a, C> {
    pub(crate) fn new(client: &'a C, domain: impl Into<String>, root: Option<Uuid>) -> Self {
        Self {
            client,
            domain: domain.into(),
            root,
            correlation_id: None,
            sequence: 0,
            type_url: None,
            payload: None,
        }
    }

    /// Set the correlation ID for request tracing.
    /// If not set, a random UUID will be generated.
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    /// Set the expected sequence number for optimistic locking.
    pub fn with_sequence(mut self, seq: u32) -> Self {
        self.sequence = seq;
        self
    }

    /// Set the command type URL and message.
    pub fn with_command<M: Message>(mut self, type_url: impl Into<String>, message: &M) -> Self {
        self.type_url = Some(type_url.into());
        self.payload = Some(message.encode_to_vec());
        self
    }

    /// Build the CommandBook without executing.
    pub fn build(self) -> Result<CommandBook> {
        self.build_inner()
    }

    fn build_inner(&self) -> Result<CommandBook> {
        let type_url = self
            .type_url
            .clone()
            .ok_or_else(|| ClientError::InvalidArgument("command type_url not set".to_string()))?;
        let payload = self
            .payload
            .clone()
            .ok_or_else(|| ClientError::InvalidArgument("command payload not set".to_string()))?;

        let correlation_id = self
            .correlation_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        Ok(CommandBook {
            cover: Some(Cover {
                domain: self.domain.clone(),
                root: self.root.map(uuid_to_proto),
                correlation_id,
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence: self.sequence,
                merge_strategy: crate::proto::MergeStrategy::MergeCommutative as i32,
                payload: Some(crate::proto::command_page::Payload::Command(prost_types::Any {
                    type_url,
                    value: payload,
                })),
            }],
            saga_origin: None,
        })
    }

    /// Execute the command.
    pub async fn execute(self) -> Result<CommandResponse> {
        let client = self.client;
        let command = self.build_inner()?;
        client.execute(command).await
    }
}

/// Builder for constructing and executing queries.
pub struct QueryBuilder<'a, C: traits::QueryClient> {
    client: &'a C,
    domain: String,
    root: Option<Uuid>,
    correlation_id: Option<String>,
    selection: Option<Selection>,
    edition: Option<String>,
}

impl<'a, C: traits::QueryClient> QueryBuilder<'a, C> {
    pub(crate) fn new(client: &'a C, domain: impl Into<String>, root: Option<Uuid>) -> Self {
        Self {
            client,
            domain: domain.into(),
            root,
            correlation_id: None,
            selection: None,
            edition: None,
        }
    }

    /// Query by correlation ID instead of root.
    pub fn by_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self.root = None;
        self
    }

    /// Query events from a specific edition (diverged timeline).
    pub fn edition(mut self, edition: impl Into<String>) -> Self {
        self.edition = Some(edition.into());
        self
    }

    /// Query a range of sequences (inclusive lower bound).
    pub fn range(mut self, lower: u32) -> Self {
        self.selection = Some(Selection::Range(SequenceRange { lower, upper: None }));
        self
    }

    /// Query a range of sequences with upper bound (inclusive).
    pub fn range_to(mut self, lower: u32, upper: u32) -> Self {
        self.selection = Some(Selection::Range(SequenceRange {
            lower,
            upper: Some(upper),
        }));
        self
    }

    /// Query state as of a specific sequence number.
    pub fn as_of_sequence(mut self, seq: u32) -> Self {
        self.selection = Some(Selection::Temporal(TemporalQuery {
            point_in_time: Some(PointInTime::AsOfSequence(seq)),
        }));
        self
    }

    /// Query state as of a specific timestamp (RFC3339 format).
    pub fn as_of_time(mut self, rfc3339: &str) -> Result<Self> {
        let timestamp = parse_timestamp(rfc3339)?;
        self.selection = Some(Selection::Temporal(TemporalQuery {
            point_in_time: Some(PointInTime::AsOfTime(timestamp)),
        }));
        Ok(self)
    }

    /// Build the Query without executing.
    pub fn build(self) -> Query {
        self.build_inner()
    }

    fn build_inner(&self) -> Query {
        Query {
            cover: Some(Cover {
                domain: self.domain.clone(),
                root: self.root.map(uuid_to_proto),
                correlation_id: self.correlation_id.clone().unwrap_or_default(),
                edition: self.edition.clone().map(Edition::from),
            }),
            selection: self.selection.clone(),
        }
    }

    /// Execute the query and return the EventBook.
    pub async fn get_events(self) -> Result<EventBook> {
        let client = self.client;
        let query = self.build_inner();
        client.get_events(query).await
    }

    /// Execute the query and return just the event pages.
    pub async fn get_pages(self) -> Result<Vec<EventPage>> {
        let client = self.client;
        let query = self.build_inner();
        let event_book = client.get_events(query).await?;
        Ok(event_book.pages)
    }
}

/// Extension trait for creating command builders.
pub trait CommandBuilderExt: traits::GatewayClient + Sized {
    /// Start building a command for the given domain and root.
    fn command(&self, domain: impl Into<String>, root: Uuid) -> CommandBuilder<'_, Self> {
        CommandBuilder::new(self, domain, Some(root))
    }

    /// Start building a command for a new aggregate (no root yet).
    fn command_new(&self, domain: impl Into<String>) -> CommandBuilder<'_, Self> {
        CommandBuilder::new(self, domain, None)
    }
}

impl<T: traits::GatewayClient> CommandBuilderExt for T {}

/// Extension trait for creating query builders.
pub trait QueryBuilderExt: traits::QueryClient + Sized {
    /// Start building a query for the given domain and root.
    fn query(&self, domain: impl Into<String>, root: Uuid) -> QueryBuilder<'_, Self> {
        QueryBuilder::new(self, domain, Some(root))
    }

    /// Start building a query by domain only (use with by_correlation_id).
    fn query_domain(&self, domain: impl Into<String>) -> QueryBuilder<'_, Self> {
        QueryBuilder::new(self, domain, None)
    }
}

impl<T: traits::QueryClient> QueryBuilderExt for T {}

/// Helper to extract the root UUID from a Cover.
pub fn root_from_cover(cover: &Cover) -> Option<Uuid> {
    cover
        .root
        .as_ref()
        .and_then(|r| Uuid::from_slice(&r.value).ok())
}

/// Helper to extract events from a CommandResponse.
pub fn events_from_response(response: &CommandResponse) -> &[EventPage] {
    response
        .events
        .as_ref()
        .map(|e| e.pages.as_slice())
        .unwrap_or(&[])
}

/// Helper to decode an event payload if the type URL matches.
pub fn decode_event<M: Message + Default>(event: &EventPage, type_suffix: &str) -> Option<M> {
    let any = match &event.payload {
        Some(crate::proto::event_page::Payload::Event(e)) => e,
        _ => return None,
    };
    if !any.type_url.ends_with(type_suffix) {
        return None;
    }
    M::decode(any.value.as_slice()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{Cover, Uuid as ProtoUuid};
    use async_trait::async_trait;

    // Mock client for testing QueryBuilder
    struct MockQueryClient {
        event_book: EventBook,
    }

    #[async_trait]
    impl traits::QueryClient for MockQueryClient {
        async fn get_events(&self, _query: Query) -> Result<EventBook> {
            Ok(self.event_book.clone())
        }
    }

    // Mock client for testing CommandBuilder
    struct MockGatewayClient {
        response: CommandResponse,
    }

    #[async_trait]
    impl traits::GatewayClient for MockGatewayClient {
        async fn execute(&self, _command: CommandBook) -> Result<CommandResponse> {
            Ok(self.response.clone())
        }
    }

    fn make_cover(domain: &str, correlation_id: &str, root: Option<Uuid>) -> Cover {
        Cover {
            domain: domain.to_string(),
            correlation_id: correlation_id.to_string(),
            root: root.map(|u| ProtoUuid {
                value: u.as_bytes().to_vec(),
            }),
            edition: None,
        }
    }

    // CommandBuilder tests
    #[test]
    fn test_command_builder_with_correlation_id() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let root = Uuid::new_v4();
        let builder =
            CommandBuilder::new(&client, "orders", Some(root)).with_correlation_id("corr-123");

        assert_eq!(builder.correlation_id, Some("corr-123".to_string()));
    }

    #[test]
    fn test_command_builder_with_sequence() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let builder = CommandBuilder::new(&client, "orders", None).with_sequence(42);

        assert_eq!(builder.sequence, 42);
    }

    #[test]
    fn test_command_builder_with_command() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let msg = prost_types::Duration {
            seconds: 42,
            nanos: 0,
        };
        let builder = CommandBuilder::new(&client, "orders", None)
            .with_command("type.googleapis.com/test.Command", &msg);

        assert_eq!(
            builder.type_url,
            Some("type.googleapis.com/test.Command".to_string())
        );
        assert!(builder.payload.is_some());
    }

    #[test]
    fn test_command_builder_build_success() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let root = Uuid::new_v4();
        let msg = prost_types::Duration {
            seconds: 42,
            nanos: 0,
        };
        let cmd = CommandBuilder::new(&client, "orders", Some(root))
            .with_correlation_id("corr-123")
            .with_sequence(5)
            .with_command("type.googleapis.com/test.Command", &msg)
            .build()
            .unwrap();

        let cover = cmd.cover.unwrap();
        assert_eq!(cover.domain, "orders");
        assert_eq!(cover.correlation_id, "corr-123");
        assert!(cover.root.is_some());
        assert_eq!(cmd.pages.len(), 1);
        assert_eq!(cmd.pages[0].sequence, 5);
    }

    #[test]
    fn test_command_builder_build_generates_correlation_id() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let msg = prost_types::Duration {
            seconds: 42,
            nanos: 0,
        };
        let cmd = CommandBuilder::new(&client, "orders", None)
            .with_command("type.googleapis.com/test.Command", &msg)
            .build()
            .unwrap();

        let cover = cmd.cover.unwrap();
        assert!(!cover.correlation_id.is_empty());
    }

    #[test]
    fn test_command_builder_build_missing_type_url() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let result = CommandBuilder::new(&client, "orders", None).build();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.is_invalid_argument());
    }

    #[test]
    fn test_command_builder_build_missing_payload() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let mut builder = CommandBuilder::new(&client, "orders", None);
        builder.type_url = Some("type.googleapis.com/test".to_string());
        let result = builder.build();

        assert!(result.is_err());
    }

    // QueryBuilder tests
    #[test]
    fn test_query_builder_by_correlation_id() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let root = Uuid::new_v4();
        let builder =
            QueryBuilder::new(&client, "orders", Some(root)).by_correlation_id("corr-123");

        assert_eq!(builder.correlation_id, Some("corr-123".to_string()));
        assert!(builder.root.is_none()); // root should be cleared
    }

    #[test]
    fn test_query_builder_edition() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let builder = QueryBuilder::new(&client, "orders", None).edition("test-edition");

        assert_eq!(builder.edition, Some("test-edition".to_string()));
    }

    #[test]
    fn test_query_builder_range() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let builder = QueryBuilder::new(&client, "orders", None).range(10);

        match builder.selection {
            Some(Selection::Range(r)) => {
                assert_eq!(r.lower, 10);
                assert!(r.upper.is_none());
            }
            _ => panic!("expected Range selection"),
        }
    }

    #[test]
    fn test_query_builder_range_to() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let builder = QueryBuilder::new(&client, "orders", None).range_to(5, 15);

        match builder.selection {
            Some(Selection::Range(r)) => {
                assert_eq!(r.lower, 5);
                assert_eq!(r.upper, Some(15));
            }
            _ => panic!("expected Range selection"),
        }
    }

    #[test]
    fn test_query_builder_as_of_sequence() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let builder = QueryBuilder::new(&client, "orders", None).as_of_sequence(42);

        match builder.selection {
            Some(Selection::Temporal(t)) => match t.point_in_time {
                Some(PointInTime::AsOfSequence(s)) => assert_eq!(s, 42),
                _ => panic!("expected AsOfSequence"),
            },
            _ => panic!("expected Temporal selection"),
        }
    }

    #[test]
    fn test_query_builder_as_of_time_valid() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let builder = QueryBuilder::new(&client, "orders", None)
            .as_of_time("2024-01-15T10:30:00Z")
            .unwrap();

        match builder.selection {
            Some(Selection::Temporal(t)) => match t.point_in_time {
                Some(PointInTime::AsOfTime(ts)) => assert_eq!(ts.seconds, 1705314600),
                _ => panic!("expected AsOfTime"),
            },
            _ => panic!("expected Temporal selection"),
        }
    }

    #[test]
    fn test_query_builder_as_of_time_invalid() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let result = QueryBuilder::new(&client, "orders", None).as_of_time("not a timestamp");

        assert!(result.is_err());
    }

    #[test]
    fn test_query_builder_build() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let root = Uuid::new_v4();
        let query = QueryBuilder::new(&client, "orders", Some(root))
            .edition("test-edition")
            .range(10)
            .build();

        let cover = query.cover.unwrap();
        assert_eq!(cover.domain, "orders");
        assert!(cover.root.is_some());
        assert!(cover.edition.is_some());
        assert!(query.selection.is_some());
    }

    #[test]
    fn test_query_builder_build_with_correlation_id() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let query = QueryBuilder::new(&client, "orders", None)
            .by_correlation_id("corr-123")
            .build();

        let cover = query.cover.unwrap();
        assert_eq!(cover.correlation_id, "corr-123");
        assert!(cover.root.is_none());
    }

    // Helper function tests
    #[test]
    fn test_root_from_cover_some() {
        let root = Uuid::new_v4();
        let cover = make_cover("orders", "", Some(root));
        assert_eq!(root_from_cover(&cover), Some(root));
    }

    #[test]
    fn test_root_from_cover_none() {
        let cover = make_cover("orders", "", None);
        assert_eq!(root_from_cover(&cover), None);
    }

    #[test]
    fn test_root_from_cover_invalid_uuid() {
        let cover = Cover {
            domain: "orders".to_string(),
            correlation_id: String::new(),
            root: Some(ProtoUuid {
                value: vec![1, 2, 3], // invalid - not 16 bytes
            }),
            edition: None,
        };
        assert_eq!(root_from_cover(&cover), None);
    }

    #[test]
    fn test_events_from_response_with_events() {
        let events = EventBook {
            cover: None,
            pages: vec![EventPage::default(), EventPage::default()],
            snapshot: None,
            next_sequence: 0,
        };
        let response = CommandResponse {
            events: Some(events),
            ..Default::default()
        };

        let pages = events_from_response(&response);
        assert_eq!(pages.len(), 2);
    }

    #[test]
    fn test_events_from_response_no_events() {
        let response = CommandResponse {
            events: None,
            ..Default::default()
        };

        let pages = events_from_response(&response);
        assert!(pages.is_empty());
    }

    #[test]
    fn test_decode_event_success() {
        use crate::proto::event_page::Payload;

        // Use prost_types::Duration which implements Message + Default
        let msg = prost_types::Duration {
            seconds: 42,
            nanos: 0,
        };
        let event = EventPage {
            sequence: 1,
            created_at: None,
            payload: Some(Payload::Event(prost_types::Any {
                type_url: "type.googleapis.com/google.protobuf.Duration".to_string(),
                value: msg.encode_to_vec(),
            })),
        };

        let decoded: Option<prost_types::Duration> = decode_event(&event, "Duration");
        assert!(decoded.is_some());
        assert_eq!(decoded.unwrap().seconds, 42);
    }

    #[test]
    fn test_decode_event_type_mismatch() {
        use crate::proto::event_page::Payload;

        let msg = prost_types::Duration {
            seconds: 42,
            nanos: 0,
        };
        let event = EventPage {
            sequence: 1,
            created_at: None,
            payload: Some(Payload::Event(prost_types::Any {
                type_url: "type.googleapis.com/google.protobuf.Duration".to_string(),
                value: msg.encode_to_vec(),
            })),
        };

        let decoded: Option<prost_types::Duration> = decode_event(&event, "Timestamp");
        assert!(decoded.is_none());
    }

    #[test]
    fn test_decode_event_nil_event() {
        let event = EventPage {
            sequence: 1,
            created_at: None,
            payload: None,
        };

        let decoded: Option<prost_types::Duration> = decode_event(&event, "Duration");
        assert!(decoded.is_none());
    }

    #[test]
    fn test_decode_event_invalid_payload() {
        use crate::proto::event_page::Payload;

        let event = EventPage {
            sequence: 1,
            created_at: None,
            payload: Some(Payload::Event(prost_types::Any {
                type_url: "type.googleapis.com/google.protobuf.Duration".to_string(),
                value: vec![0xFF, 0xFF, 0xFF], // garbage
            })),
        };

        let decoded: Option<prost_types::Duration> = decode_event(&event, "Duration");
        assert!(decoded.is_none());
    }

    // Extension trait tests
    #[test]
    fn test_command_builder_ext_command() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let root = Uuid::new_v4();
        let builder = client.command("orders", root);

        assert_eq!(builder.domain, "orders");
        assert_eq!(builder.root, Some(root));
    }

    #[test]
    fn test_command_builder_ext_command_new() {
        let client = MockGatewayClient {
            response: CommandResponse::default(),
        };
        let builder = client.command_new("orders");

        assert_eq!(builder.domain, "orders");
        assert!(builder.root.is_none());
    }

    #[test]
    fn test_query_builder_ext_query() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let root = Uuid::new_v4();
        let builder = client.query("orders", root);

        assert_eq!(builder.domain, "orders");
        assert_eq!(builder.root, Some(root));
    }

    #[test]
    fn test_query_builder_ext_query_domain() {
        let client = MockQueryClient {
            event_book: EventBook::default(),
        };
        let builder = client.query_domain("orders");

        assert_eq!(builder.domain, "orders");
        assert!(builder.root.is_none());
    }
}
