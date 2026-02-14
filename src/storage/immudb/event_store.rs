//! ImmuDB EventStore implementation via PostgreSQL wire protocol.
//!
//! Uses sqlx with Postgres driver connecting to immudb's pgsql server.
//! Queries built with sea_query for type-safe SQL generation.

use async_trait::async_trait;
use prost::Message;
use sea_query::{Expr, Order, PostgresQueryBuilder, Query};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::EventPage;
use crate::storage::helpers::{assemble_event_books, is_main_timeline};
use crate::storage::schema::Events;
use crate::storage::{EventStore, Result, StorageError};

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
    pub async fn init_schema(&self) -> Result<()> {
        sqlx::query(super::schema::CREATE_EVENTS_TABLE)
            .execute(&self.pool)
            .await?;

        // Note: immudb requires indexes on empty tables, so these may fail
        // if table already has data. Using IF NOT EXISTS to handle gracefully.
        let _ = sqlx::query(super::schema::CREATE_CORRELATION_INDEX)
            .execute(&self.pool)
            .await;

        let _ = sqlx::query(super::schema::CREATE_DOMAIN_ROOT_INDEX)
            .execute(&self.pool)
            .await;

        Ok(())
    }

    /// Query events for a specific edition.
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

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get minimum sequence from edition events (implicit divergence point).
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

        let row = sqlx::query(&query).fetch_optional(&self.pool).await?;

        match row {
            Some(row) => {
                let min_seq: Option<i64> = row.get(0);
                Ok(min_seq.map(|s| s as u32))
            }
            None => Ok(None),
        }
    }

    /// Query main timeline events up to (but not including) a sequence.
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

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
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

        let row = sqlx::query(&query).fetch_optional(&self.pool).await?;

        match row {
            Some(row) => {
                let max_seq: Option<i64> = row.get(0);
                Ok(max_seq.map(|s| s as u32))
            }
            None => Ok(None),
        }
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

            let query = Query::insert()
                .into_table(Events::Table)
                .columns([
                    Events::Edition,
                    Events::Domain,
                    Events::Root,
                    Events::Sequence,
                    Events::CreatedAt,
                    Events::EventData,
                    Events::CorrelationId,
                ])
                .values_panic([
                    edition.into(),
                    domain.into(),
                    root_str.clone().into(),
                    sequence.into(),
                    created_at.into(),
                    event_data.into(),
                    correlation_id.into(),
                ])
                .to_string(PostgresQueryBuilder);

            sqlx::query(&query).execute(&self.pool).await?;
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
            .and_where(Expr::col(Events::Sequence).lte(to))
            .order_by(Events::Sequence, Order::Asc)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
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

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
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

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut books_map = std::collections::HashMap::new();

        for row in rows {
            let domain: String = row.get("domain");
            let edition: String = row.get("edition");
            let root_str: String = row.get("root");
            let event_data: Vec<u8> = row.get("event_data");

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

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut roots = Vec::with_capacity(rows.len());
        for row in rows {
            let root_str: String = row.get("root");
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

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut domains = Vec::with_capacity(rows.len());
        for row in rows {
            let domain: String = row.get("domain");
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
