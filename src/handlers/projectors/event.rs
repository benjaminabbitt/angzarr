//! Generic Event projector.
//!
//! Writes all events as JSON to a database table for querying and debugging.
//! Each event is keyed by (domain, root_id, sequence).

use std::sync::Arc;

use prost_reflect::{DescriptorPool, DynamicMessage};
use sea_query::{ColumnDef, Index, OnConflict, Query, Table};
use tonic::{Request, Response, Status};
use tracing::{debug, error, info, warn};

use crate::proto::projector_coordinator_service_server::ProjectorCoordinatorService;
use crate::proto::{EventBook, Projection, SpeculateProjectorRequest, SyncEventBook};

// Database backend selection via features
#[cfg(feature = "postgres")]
use sea_query::PostgresQueryBuilder;
#[cfg(feature = "postgres")]
use sqlx::PgPool;

#[cfg(feature = "sqlite")]
use sea_query::SqliteQueryBuilder;
#[cfg(feature = "sqlite")]
use sqlx::SqlitePool;

// Type aliases for database-agnostic code
#[cfg(feature = "postgres")]
pub type Pool = PgPool;

#[cfg(feature = "sqlite")]
pub type Pool = SqlitePool;

/// Build schema SQL using the appropriate backend.
#[cfg(feature = "postgres")]
fn build_schema<T: sea_query::SchemaStatementBuilder>(stmt: T) -> String {
    stmt.to_string(PostgresQueryBuilder)
}

#[cfg(feature = "sqlite")]
fn build_schema<T: sea_query::SchemaStatementBuilder>(stmt: T) -> String {
    stmt.to_string(SqliteQueryBuilder)
}

/// Build query SQL using the appropriate backend.
#[cfg(feature = "postgres")]
fn build_query<T: sea_query::QueryStatementWriter>(stmt: T) -> String {
    stmt.to_string(PostgresQueryBuilder)
}

#[cfg(feature = "sqlite")]
fn build_query<T: sea_query::QueryStatementWriter>(stmt: T) -> String {
    stmt.to_string(SqliteQueryBuilder)
}

/// Table and column identifiers for sea-query.
#[derive(sea_query::Iden)]
enum Events {
    Table,
    Domain,
    RootId,
    Sequence,
    EventType,
    EventJson,
    CorrelationId,
    CreatedAt,
}

/// Single event record for storage.
struct EventRecord<'a> {
    domain: &'a str,
    root_id: &'a str,
    sequence: u32,
    event_type: &'a str,
    event_json: &'a str,
    correlation_id: &'a str,
    created_at: &'a str,
}

impl EventRecord<'_> {
    /// Build an INSERT statement for this record.
    ///
    /// Uses ON CONFLICT DO NOTHING for idempotency (works on both Postgres and SQLite).
    fn build_insert(&self) -> Result<sea_query::InsertStatement, sea_query::error::Error> {
        let mut stmt = Query::insert();
        stmt.into_table(Events::Table)
            .columns([
                Events::Domain,
                Events::RootId,
                Events::Sequence,
                Events::EventType,
                Events::EventJson,
                Events::CorrelationId,
                Events::CreatedAt,
            ])
            .values([
                self.domain.into(),
                self.root_id.into(),
                (self.sequence as i32).into(),
                self.event_type.into(),
                self.event_json.into(),
                self.correlation_id.into(),
                self.created_at.into(),
            ])?
            .on_conflict(
                OnConflict::columns([Events::Domain, Events::RootId, Events::Sequence])
                    .do_nothing()
                    .to_owned(),
            );

        Ok(stmt)
    }
}

/// Generic Event projector service.
///
/// Writes all events as JSON to a database for querying.
pub struct EventService {
    pool: Pool,
    descriptor_pool: Option<DescriptorPool>,
}

impl EventService {
    /// Create a new event service with the given database pool.
    pub fn new(pool: Pool) -> Self {
        Self {
            pool,
            descriptor_pool: None,
        }
    }

    /// Create with protobuf descriptors for JSON decoding.
    pub fn with_descriptors(pool: Pool, descriptor_pool: DescriptorPool) -> Self {
        Self {
            pool,
            descriptor_pool: Some(descriptor_pool),
        }
    }

    /// Load descriptors from a file path.
    pub fn load_descriptors(mut self, path: &str) -> Self {
        match std::fs::read(path) {
            Ok(bytes) => match DescriptorPool::decode(bytes.as_slice()) {
                Ok(pool) => {
                    info!(
                        path = %path,
                        message_count = pool.all_messages().count(),
                        "Loaded protobuf descriptors"
                    );
                    self.descriptor_pool = Some(pool);
                }
                Err(e) => {
                    warn!(error = %e, path = %path, "Failed to decode descriptor set");
                }
            },
            Err(e) => {
                warn!(error = %e, path = %path, "Failed to read descriptor file");
            }
        }
        self
    }

    /// Initialize database schema.
    pub async fn init(&self) -> Result<(), sqlx::Error> {
        let create_table = build_schema(
            Table::create()
                .table(Events::Table)
                .if_not_exists()
                .col(ColumnDef::new(Events::Domain).text().not_null())
                .col(ColumnDef::new(Events::RootId).text().not_null())
                .col(ColumnDef::new(Events::Sequence).integer().not_null())
                .col(ColumnDef::new(Events::EventType).text().not_null())
                .col(ColumnDef::new(Events::EventJson).text().not_null())
                .col(ColumnDef::new(Events::CorrelationId).text().not_null())
                .col(ColumnDef::new(Events::CreatedAt).text().not_null())
                .primary_key(
                    Index::create()
                        .col(Events::Domain)
                        .col(Events::RootId)
                        .col(Events::Sequence),
                )
                .to_owned(),
        );

        sqlx::query(&create_table).execute(&self.pool).await?;

        // Index on domain + event_type for filtering by event type
        let idx_domain_type = build_schema(
            Index::create()
                .if_not_exists()
                .name("idx_events_domain_type")
                .table(Events::Table)
                .col(Events::Domain)
                .col(Events::EventType)
                .to_owned(),
        );

        sqlx::query(&idx_domain_type).execute(&self.pool).await?;

        // Index on correlation_id for tracing flows
        let idx_correlation = build_schema(
            Index::create()
                .if_not_exists()
                .name("idx_events_correlation")
                .table(Events::Table)
                .col(Events::CorrelationId)
                .to_owned(),
        );

        sqlx::query(&idx_correlation).execute(&self.pool).await?;

        // Index on created_at for time-based queries
        let idx_created = build_schema(
            Index::create()
                .if_not_exists()
                .name("idx_events_created")
                .table(Events::Table)
                .col(Events::CreatedAt)
                .to_owned(),
        );

        sqlx::query(&idx_created).execute(&self.pool).await?;

        info!("Events table schema initialized");
        Ok(())
    }

    /// Decode an event to JSON string.
    fn decode_event(&self, any: &prost_types::Any) -> String {
        let type_name = any.type_url.rsplit('/').next().unwrap_or(&any.type_url);

        if let Some(pool) = &self.descriptor_pool {
            if let Some(desc) = pool.get_message_by_name(type_name) {
                if let Ok(msg) = DynamicMessage::decode(desc, any.value.as_slice()) {
                    if let Ok(json) = serde_json::to_string(&msg) {
                        return json;
                    }
                }
            }
        }

        // Fallback: base64-encoded binary with type info
        serde_json::json!({
            "_type": any.type_url,
            "_binary": base64_encode(&any.value),
            "_size": any.value.len()
        })
        .to_string()
    }

    /// Extract event type from type_url.
    fn extract_event_type(type_url: &str) -> &str {
        type_url.rsplit('/').next().unwrap_or(type_url)
    }

    /// Store a single event.
    ///
    /// Uses sea-query for type-safe query building with proper escaping.
    async fn store_event(&self, record: &EventRecord<'_>) -> Result<(), sqlx::Error> {
        let stmt = record
            .build_insert()
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        let query = build_query(stmt);
        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }

    /// Handle an event book by storing all events.
    pub async fn handle_book(&self, book: &EventBook) -> Result<(), Status> {
        let cover = match &book.cover {
            Some(c) => c,
            None => {
                warn!("EventBook missing cover");
                return Ok(());
            }
        };

        let domain = &cover.domain;
        let root_id = cover
            .root
            .as_ref()
            .map(|u| hex::encode(&u.value))
            .unwrap_or_else(|| "unknown".to_string());

        let correlation_id = &cover.correlation_id;

        for page in &book.pages {
            let sequence = match &page.sequence {
                Some(crate::proto::event_page::Sequence::Num(n)) => *n,
                Some(crate::proto::event_page::Sequence::Force(_)) => continue,
                None => continue,
            };

            let Some(event) = &page.event else {
                continue;
            };

            let event_type = Self::extract_event_type(&event.type_url);
            let event_json = self.decode_event(event);
            let created_at = page
                .created_at
                .as_ref()
                .map(|ts| {
                    chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32)
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_else(|| chrono::Utc::now().to_rfc3339())
                })
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let record = EventRecord {
                domain,
                root_id: &root_id,
                sequence,
                event_type,
                event_json: &event_json,
                correlation_id,
                created_at: &created_at,
            };

            if let Err(e) = self.store_event(&record).await {
                error!(
                    error = %e,
                    domain = %domain,
                    root_id = %root_id,
                    sequence = sequence,
                    event_type = %event_type,
                    "Failed to store event"
                );
            } else {
                debug!(
                    domain = %domain,
                    root_id = %root_id,
                    sequence = sequence,
                    event_type = %event_type,
                    "Event stored"
                );
            }
        }

        Ok(())
    }
}

/// Base64 encode bytes.
fn base64_encode(bytes: &[u8]) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut result = String::new();

    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;

        result.push(ALPHABET[b0 >> 2] as char);
        result.push(ALPHABET[((b0 & 0x03) << 4) | (b1 >> 4)] as char);

        if chunk.len() > 1 {
            result.push(ALPHABET[((b1 & 0x0f) << 2) | (b2 >> 6)] as char);
        } else {
            result.push('=');
        }

        if chunk.len() > 2 {
            result.push(ALPHABET[b2 & 0x3f] as char);
        } else {
            result.push('=');
        }
    }

    result
}

#[tonic::async_trait]
impl ProjectorCoordinatorService for EventService {
    async fn handle_sync(
        &self,
        request: Request<SyncEventBook>,
    ) -> Result<Response<Projection>, Status> {
        if let Some(book) = request.into_inner().events {
            self.handle_book(&book).await?;
        }
        Ok(Response::new(Projection::default()))
    }

    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        let book = request.into_inner();
        self.handle_book(&book).await?;
        Ok(Response::new(()))
    }

    async fn handle_speculative(
        &self,
        request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        // Event store projector doesn't produce a read model projection
        if let Some(book) = request.into_inner().events {
            self.handle_book(&book).await?;
        }
        Ok(Response::new(Projection::default()))
    }
}

/// Wrapper to share EventService across async contexts.
#[derive(Clone)]
pub struct EventServiceHandle(pub Arc<EventService>);

impl std::ops::Deref for EventServiceHandle {
    type Target = EventService;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[tonic::async_trait]
impl ProjectorCoordinatorService for EventServiceHandle {
    async fn handle_sync(
        &self,
        request: Request<SyncEventBook>,
    ) -> Result<Response<Projection>, Status> {
        ProjectorCoordinatorService::handle_sync(&*self.0, request).await
    }

    async fn handle(&self, request: Request<EventBook>) -> Result<Response<()>, Status> {
        ProjectorCoordinatorService::handle(&*self.0, request).await
    }

    async fn handle_speculative(
        &self,
        request: Request<SpeculateProjectorRequest>,
    ) -> Result<Response<Projection>, Status> {
        ProjectorCoordinatorService::handle_speculative(&*self.0, request).await
    }
}

/// Connect to the database using the appropriate backend.
#[cfg(feature = "postgres")]
pub async fn connect_pool(database_url: &str) -> Result<Pool, sqlx::Error> {
    sqlx::postgres::PgPool::connect(database_url).await
}

#[cfg(feature = "sqlite")]
pub async fn connect_pool(database_url: &str) -> Result<Pool, sqlx::Error> {
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(database_url.trim_start_matches("sqlite:"))
        .create_if_missing(true);
    sqlx::sqlite::SqlitePool::connect_with(options).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base64_encode() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"foob"), "Zm9vYg==");
        assert_eq!(base64_encode(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64_encode(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn test_extract_event_type() {
        assert_eq!(
            EventService::extract_event_type("type.googleapis.com/orders.OrderCreated"),
            "orders.OrderCreated"
        );
        assert_eq!(
            EventService::extract_event_type("OrderCreated"),
            "OrderCreated"
        );
    }

    #[test]
    fn test_event_record_build_insert() {
        let record = EventRecord {
            domain: "orders",
            root_id: "abc123",
            sequence: 42,
            event_type: "OrderCreated",
            event_json: r#"{"id": 1}"#,
            correlation_id: "corr-456",
            created_at: "2024-01-01T00:00:00Z",
        };

        let stmt = record.build_insert().expect("should build insert");
        let sql = build_query(stmt);

        assert!(sql.contains("INSERT INTO"));
        assert!(sql.contains("events"));
        assert!(sql.contains("orders"));
        assert!(sql.contains("abc123"));
        assert!(sql.contains("42"));
        assert!(sql.contains("OrderCreated"));
    }

    #[test]
    fn test_event_record_escapes_special_characters() {
        let record = EventRecord {
            domain: "test'; DROP TABLE events;--",
            root_id: "root",
            sequence: 1,
            event_type: "Event",
            event_json: "{}",
            correlation_id: "corr",
            created_at: "2024-01-01T00:00:00Z",
        };

        let stmt = record.build_insert().expect("should build insert");
        let sql = build_query(stmt);

        // Sea-query escapes single quotes by doubling them, making injection impossible.
        // The text "DROP TABLE" still appears as a literal string value, but it cannot
        // execute because the surrounding quotes prevent SQL statement termination.
        assert!(sql.contains("''"), "single quotes should be escaped");
        assert!(
            sql.contains("test''; DROP TABLE"),
            "injection payload should be safely escaped within string literal"
        );
    }
}
