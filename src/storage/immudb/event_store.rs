//! ImmuDB EventStore implementation via PostgreSQL wire protocol.
//!
//! Uses sqlx with Postgres driver connecting to immudb's pgsql server.
//! Queries built with sea_query for type-safe SQL generation.
//!
//! # Simple Query Mode
//!
//! immudb's pgsql server only supports simple query mode - it does not support
//! the extended query protocol (prepared statements). All queries must be
//! executed using `raw_sql()` to avoid Parse/Bind/Execute messages.

use async_trait::async_trait;
use hex;
use prost::Message;
use sea_query::{Asterisk, Expr, Order, PostgresQueryBuilder, Query};
use sqlx::{Executor, PgPool, Row};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::EventPage;
use crate::storage::helpers::{assemble_event_books, is_main_timeline};
use crate::storage::schema::Events;
use crate::storage::{EventStore, Result, StorageError};

/// Decode a BLOB column from immudb.
///
/// immudb returns BLOBs as hex-encoded ASCII strings through the pgsql wire
/// protocol, not as raw bytes. We need to decode the hex string.
fn decode_blob_column(row: &sqlx::postgres::PgRow, index: usize) -> Result<Vec<u8>> {
    use sqlx::Row as _;
    use sqlx::ValueRef;

    // Get the raw column value
    let value_ref = row.try_get_raw(index)?;

    // Check if it's null
    if value_ref.is_null() {
        return Ok(Vec::new());
    }

    // immudb returns BLOB as hex-encoded ASCII string bytes
    let hex_bytes = value_ref.as_bytes().map_err(|e| {
        StorageError::InvalidTimestampFormat(format!("failed to get raw bytes: {}", e))
    })?;

    // Convert ASCII bytes to string and decode hex
    let hex_str = std::str::from_utf8(hex_bytes).map_err(|e| {
        StorageError::InvalidTimestampFormat(format!("invalid UTF-8 in hex string: {}", e))
    })?;

    // Decode the hex string to get the original binary data
    hex::decode(hex_str)
        .map_err(|e| StorageError::InvalidTimestampFormat(format!("hex decode error: {}", e)))
}

/// ImmuDB implementation of EventStore via pgsql wire protocol.
///
/// Connects to immudb using standard Postgres driver. immudb must be
/// started with `IMMUDB_PGSQL_SERVER=true`.
///
/// # Connection String
///
/// ```text
/// postgresql://immudb:immudb@localhost:5432/defaultdb?sslmode=disable
/// ```
///
/// # Advantages over other backends
///
/// - **Immutability guaranteed**: immudb prevents modification/deletion at storage level
/// - **Cryptographic proofs**: Data integrity verifiable via Merkle trees
/// - **Time-travel**: `SINCE TX` queries for temporal access
/// - **Audit trail**: `HISTORY OF events` for full revision history
pub struct ImmudbEventStore {
    pool: PgPool,
}

impl ImmudbEventStore {
    /// Create a new immudb event store.
    ///
    /// The pool should be configured to connect to immudb's pgsql port.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initialize the schema (create tables and indexes).
    ///
    /// Safe to call multiple times - uses IF NOT EXISTS.
    /// Uses raw_sql for immudb simple query mode compatibility.
    pub async fn init_schema(&self) -> Result<()> {
        self.pool
            .execute(sqlx::raw_sql(super::schema::CREATE_EVENTS_TABLE))
            .await?;

        // Note: immudb requires indexes on empty tables, so these may fail
        // if table already has data. Using IF NOT EXISTS to handle gracefully.
        let _ = self
            .pool
            .execute(sqlx::raw_sql(super::schema::CREATE_CORRELATION_INDEX))
            .await;

        let _ = self
            .pool
            .execute(sqlx::raw_sql(super::schema::CREATE_DOMAIN_ROOT_INDEX))
            .await;

        Ok(())
    }

    /// Query events for a specific edition.
    /// Uses raw_sql for immudb simple query mode compatibility.
    async fn query_edition_events(
        &self,
        domain: &str,
        edition: &str,
        root_str: &str,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(root_str))
            .and_where(Expr::col(Events::Sequence).gte(from))
            .order_by(Events::Sequence, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            // Use index 0 since raw_sql doesn't reliably support column names
            // immudb returns BLOBs as hex strings through pgsql wire protocol
            let event_data = decode_blob_column(&row, 0)?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get minimum sequence from edition events (implicit divergence point).
    /// Uses raw_sql for immudb simple query mode compatibility.
    async fn get_edition_min_sequence(
        &self,
        domain: &str,
        edition: &str,
        root_str: &str,
    ) -> Result<Option<u32>> {
        let query = Query::select()
            .expr(Expr::col(Events::Sequence).min())
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(root_str))
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let min_seq: Option<i64> = rows[0].get(0);
        Ok(min_seq.map(|s| s as u32))
    }

    /// Query main timeline events up to (but not including) a sequence.
    /// Uses raw_sql for immudb simple query mode compatibility.
    async fn query_main_events_until(
        &self,
        domain: &str,
        root_str: &str,
        until_seq: u32,
    ) -> Result<Vec<EventPage>> {
        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(DEFAULT_EDITION))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(root_str))
            .and_where(Expr::col(Events::Sequence).lt(until_seq))
            .order_by(Events::Sequence, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            // Use index 0 since raw_sql doesn't reliably support column names
            let event_data = decode_blob_column(&row, 0)?;
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    /// Composite read for editions: main timeline (before divergence) + edition events.
    async fn composite_read(
        &self,
        domain: &str,
        edition: &str,
        root_str: &str,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        // Query edition events first to find divergence point
        let edition_events = self
            .query_edition_events(domain, edition, root_str, 0)
            .await?;

        if edition_events.is_empty() {
            // No edition events - just return main timeline
            return self
                .query_edition_events(domain, DEFAULT_EDITION, root_str, from)
                .await;
        }

        // Get divergence point (first edition event's sequence)
        let divergence = self
            .get_edition_min_sequence(domain, edition, root_str)
            .await?
            .unwrap_or(0);

        // Query main timeline up to divergence
        let main_events = self
            .query_main_events_until(domain, root_str, divergence)
            .await?;

        // Merge: main events (>= from, < divergence) + edition events (>= from)
        let mut result = Vec::new();

        for event in main_events {
            let seq = crate::storage::helpers::event_sequence(&event);
            if seq >= from {
                result.push(event);
            }
        }

        for event in edition_events {
            let seq = crate::storage::helpers::event_sequence(&event);
            if seq >= from {
                result.push(event);
            }
        }

        Ok(result)
    }

    /// Get max sequence number for an aggregate.
    /// Uses raw_sql for immudb simple query mode compatibility.
    async fn get_max_sequence(
        &self,
        domain: &str,
        edition: &str,
        root_str: &str,
    ) -> Result<Option<u32>> {
        let query = Query::select()
            .expr(Expr::col(Events::Sequence).max())
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(root_str))
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        if rows.is_empty() {
            return Ok(None);
        }

        // immudb returns 0 instead of NULL for MAX() on empty result sets,
        // so we need to check if any events actually exist
        let max_seq: Option<i64> = rows[0].try_get(0).ok().flatten();

        // If we got a value, verify it's not a false 0 from empty result
        // by checking if the aggregate actually has events
        if max_seq == Some(0) {
            // Check if there's actually a sequence 0 event
            // Note: immudb only supports COUNT(*), not COUNT(column)
            let count_query = Query::select()
                .expr(Expr::col(Asterisk).count())
                .from(Events::Table)
                .and_where(Expr::col(Events::Edition).eq(edition))
                .and_where(Expr::col(Events::Domain).eq(domain))
                .and_where(Expr::col(Events::Root).eq(root_str))
                .to_string(PostgresQueryBuilder);

            let count_rows = sqlx::raw_sql(&count_query).fetch_all(&self.pool).await?;
            if !count_rows.is_empty() {
                let count: i64 = count_rows[0].try_get(0).unwrap_or(0);
                if count == 0 {
                    return Ok(None);
                }
            }
        }

        Ok(max_seq.map(|s| s as u32))
    }
}

#[async_trait]
impl EventStore for ImmudbEventStore {
    async fn add(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string();

        // Get base sequence for this aggregate
        let base_sequence = self
            .get_max_sequence(domain, edition, &root_str)
            .await?
            .map(|s| s + 1)
            .unwrap_or(0);

        let mut auto_sequence = base_sequence;

        // Insert events one by one (immudb may not support multi-row INSERT well)
        for event in events {
            let event_data = event.encode_to_vec();
            let sequence = crate::storage::helpers::resolve_sequence(
                &event,
                base_sequence,
                &mut auto_sequence,
            )?;
            let created_at = crate::storage::helpers::parse_timestamp(&event)?;

            // Format event_data as hex for immudb BLOB type (x'...' format)
            let event_data_hex = format!("x'{}'", hex::encode(&event_data));

            // Convert RFC3339 timestamp to simple format for immudb
            // immudb expects format: YYYY-MM-DD HH:MM:SS (no nanoseconds)
            let timestamp_simple = created_at
                .replace('T', " ")
                .split('+')
                .next()
                .unwrap_or(&created_at)
                .split('.')
                .next()
                .unwrap_or(&created_at)
                .to_string();

            // Build INSERT manually since sea-query doesn't handle immudb BLOB format
            // Note: immudb requires CAST for string timestamps
            let query = format!(
                "INSERT INTO events (edition, domain, root, sequence, created_at, event_data, correlation_id) \
                 VALUES ('{}', '{}', '{}', {}, CAST('{}' AS TIMESTAMP), {}, '{}')",
                edition.replace('\'', "''"),
                domain.replace('\'', "''"),
                root_str.replace('\'', "''"),
                sequence,
                timestamp_simple,
                event_data_hex,
                correlation_id.replace('\'', "''")
            );

            // Use raw_sql for immudb simple query mode compatibility
            self.pool.execute(sqlx::raw_sql(&query)).await?;
        }

        Ok(())
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.get_from(domain, edition, root, 0).await
    }

    async fn get_from(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        if is_main_timeline(edition) {
            self.query_edition_events(domain, DEFAULT_EDITION, &root_str, from)
                .await
        } else {
            self.composite_read(domain, edition, &root_str, from).await
        }
    }

    async fn get_from_to(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(if is_main_timeline(edition) {
                DEFAULT_EDITION
            } else {
                edition
            }))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(&root_str))
            .and_where(Expr::col(Events::Sequence).gte(from))
            .and_where(Expr::col(Events::Sequence).lt(to)) // exclusive end [from, to)
            .order_by(Events::Sequence, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data = decode_blob_column(&row, 0)?; // Use index for raw_sql compatibility
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    async fn get_until_timestamp(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        until: &str,
    ) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        // Use created_at filter for timestamp queries
        // Note: immudb's BEFORE TX syntax isn't available through standard SQL
        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(if is_main_timeline(edition) {
                DEFAULT_EDITION
            } else {
                edition
            }))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(&root_str))
            .and_where(Expr::col(Events::CreatedAt).lte(until))
            .order_by(Events::Sequence, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data = decode_blob_column(&row, 0)?; // Use index for raw_sql compatibility
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    async fn get_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        let query = Query::select()
            .columns([
                Events::Domain,
                Events::Edition,
                Events::Root,
                Events::EventData,
            ])
            .from(Events::Table)
            .and_where(Expr::col(Events::CorrelationId).eq(correlation_id))
            .order_by(Events::Domain, Order::Asc)
            .order_by(Events::Root, Order::Asc)
            .order_by(Events::Sequence, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        let mut books_map = std::collections::HashMap::new();

        for row in rows {
            // Columns: Domain(0), Edition(1), Root(2), EventData(3)
            let domain: String = row.get(0);
            let edition: String = row.get(1);
            let root_str: String = row.get(2);
            let event_data = decode_blob_column(&row, 3)?;

            let root = Uuid::parse_str(&root_str)?;
            let event = EventPage::decode(event_data.as_slice())?;

            books_map
                .entry((domain, edition, root))
                .or_insert_with(Vec::new)
                .push(event);
        }

        Ok(assemble_event_books(books_map, correlation_id))
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        let root_str = root.to_string();

        let max_seq = self.get_max_sequence(domain, edition, &root_str).await?;

        Ok(max_seq.map(|s| s + 1).unwrap_or(0))
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        // immudb may not support DISTINCT well, use regular query
        let query = Query::select()
            .column(Events::Root)
            .distinct()
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        let mut roots = Vec::with_capacity(rows.len());
        for row in rows {
            let root_str: String = row.get(0); // Root is the only column
            let root = Uuid::parse_str(&root_str)?;
            roots.push(root);
        }

        Ok(roots)
    }

    async fn list_domains(&self) -> Result<Vec<String>> {
        let query = Query::select()
            .column(Events::Domain)
            .distinct()
            .from(Events::Table)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::raw_sql(&query).fetch_all(&self.pool).await?;

        let mut domains = Vec::with_capacity(rows.len());
        for row in rows {
            let domain: String = row.get(0); // Domain is the only column
            domains.push(domain);
        }

        Ok(domains)
    }

    async fn delete_edition_events(&self, _domain: &str, _edition: &str) -> Result<u32> {
        // immudb is immutable - deletion is not supported by design
        // This is a feature, not a bug: events should never be deleted
        Err(StorageError::NotImplemented(
            "immudb does not support deletion - events are immutable".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    // Integration tests require a running immudb instance
    // See tests/immudb_integration/ for full test suite
}
