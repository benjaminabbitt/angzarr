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
                command: Some(prost_types::Any {
                    type_url,
                    value: payload,
                }),
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

    /// Execute the query and return a single EventBook.
    pub async fn get_event_book(self) -> Result<EventBook> {
        let client = self.client;
        let query = self.build_inner();
        client.get_event_book(query).await
    }

    /// Execute the query and return all matching EventBooks.
    pub async fn get_events(self) -> Result<Vec<EventBook>> {
        let client = self.client;
        let query = self.build_inner();
        client.get_events(query).await
    }

    /// Execute the query and return just the event pages.
    pub async fn get_pages(self) -> Result<Vec<EventPage>> {
        let client = self.client;
        let query = self.build_inner();
        let event_book = client.get_event_book(query).await?;
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
    let any = event.event.as_ref()?;
    if !any.type_url.ends_with(type_suffix) {
        return None;
    }
    M::decode(any.value.as_slice()).ok()
}
