//! SQLite EventStore implementation.
//!
//! Implements composite reads for editions: query edition events first to derive
//! the implicit divergence point, then query main timeline up to that point,
//! then merge the results.

use async_trait::async_trait;
use prost::Message;
use sea_query::{Expr, Order, Query, SqliteQueryBuilder};
use sqlx::{Row, SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::storage::schema::Events;
use crate::storage::{EventStore, Result};
use crate::proto::EventPage;

/// SQLite implementation of EventStore.
pub struct SqliteEventStore {
    pool: SqlitePool,
}

impl SqliteEventStore {
    /// Create a new SQLite event store.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Check if edition is the main timeline.
    fn is_main_timeline(edition: &str) -> bool {
        edition.is_empty() || edition == DEFAULT_EDITION
    }

    /// Query events for a specific edition (internal helper).
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
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    /// Get the minimum sequence number from edition events (implicit divergence point).
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
            .to_string(SqliteQueryBuilder);

        let row = sqlx::query(&query).fetch_optional(&self.pool).await?;

        match row {
            Some(row) => {
                let min_seq: Option<i32> = row.get(0);
                Ok(min_seq.map(|s| s as u32))
            }
            None => Ok(None),
        }
    }

    /// Query main timeline events up to (but not including) a sequence number.
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
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    /// Perform a composite read for an edition.
    ///
    /// 1. Query edition events to get implicit divergence point (min sequence)
    /// 2. Query main timeline events up to divergence point
    /// 3. Merge: main events + edition events
    async fn composite_read(
        &self,
        domain: &str,
        edition: &str,
        root_str: &str,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        // Query edition events first to determine divergence point
        let edition_events = self.query_edition_events(domain, edition, root_str, 0).await?;

        if edition_events.is_empty() {
            // No edition events - return main timeline only
            return self.query_edition_events(domain, DEFAULT_EDITION, root_str, from).await;
        }

        // Get implicit divergence point from first edition event
        let divergence = self.get_edition_min_sequence(domain, edition, root_str).await?
            .unwrap_or(0);

        // Query main timeline events up to divergence point
        let main_events = self.query_main_events_until(domain, root_str, divergence).await?;

        // Merge: main events (filtered by from) + edition events (filtered by from)
        let mut result = Vec::new();

        // Add main events that are >= from and < divergence
        for event in main_events {
            let seq = crate::storage::helpers::event_sequence(&event);
            if seq >= from {
                result.push(event);
            }
        }

        // Add edition events that are >= from
        for event in edition_events {
            let seq = crate::storage::helpers::event_sequence(&event);
            if seq >= from {
                result.push(event);
            }
        }

        Ok(result)
    }

    /// Insert events within an already-started transaction.
    async fn insert_events(
        conn: &mut SqliteConnection,
        domain: &str,
        edition: &str,
        root_str: &str,
        events: Vec<EventPage>,
        correlation_id: &str,
    ) -> Result<()> {
        let base_sequence = {
            let query = Query::select()
                .expr(Expr::col(Events::Sequence).max())
                .from(Events::Table)
                .and_where(Expr::col(Events::Edition).eq(edition))
                .and_where(Expr::col(Events::Domain).eq(domain))
                .and_where(Expr::col(Events::Root).eq(root_str))
                .to_string(SqliteQueryBuilder);

            let row = sqlx::query(&query).fetch_optional(&mut *conn).await?;

            match row {
                Some(row) => {
                    let max_seq: Option<i32> = row.get(0);
                    max_seq.map(|s| s as u32 + 1).unwrap_or(0)
                }
                None => 0,
            }
        };

        let mut auto_sequence = base_sequence;

        for event in events {
            let event_data = event.encode_to_vec();
            let sequence =
                crate::storage::helpers::resolve_sequence(&event, base_sequence, &mut auto_sequence)?;
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
                    root_str.to_string().into(),
                    sequence.into(),
                    created_at.into(),
                    event_data.into(),
                    correlation_id.into(),
                ])
                .to_string(SqliteQueryBuilder);

            sqlx::query(&query).execute(&mut *conn).await?;
        }

        Ok(())
    }
}

#[async_trait]
impl EventStore for SqliteEventStore {
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

        // BEGIN IMMEDIATE acquires the write lock upfront, preventing deadlocks
        // when concurrent DEFERRED transactions race to upgrade from shared to exclusive.
        let mut conn = self.pool.acquire().await?;
        sqlx::query("BEGIN IMMEDIATE")
            .execute(&mut *conn)
            .await?;

        let result = Self::insert_events(&mut conn, domain, edition, &root_str, events, correlation_id).await;

        match result {
            Ok(()) => {
                sqlx::query("COMMIT").execute(&mut *conn).await?;
                Ok(())
            }
            Err(e) => {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                Err(e)
            }
        }
    }

    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.get_from(domain, edition, root, 0).await
    }

    async fn get_from(&self, domain: &str, edition: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        // Main timeline: simple query
        if Self::is_main_timeline(edition) {
            return self.query_edition_events(domain, DEFAULT_EDITION, &root_str, from).await;
        }

        // Named edition: composite read (main timeline up to divergence + edition events)
        self.composite_read(domain, edition, &root_str, from).await
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
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(&root_str))
            .and_where(Expr::col(Events::Sequence).gte(from))
            .and_where(Expr::col(Events::Sequence).lt(to))
            .order_by(Events::Sequence, Order::Asc)
            .to_string(SqliteQueryBuilder);

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

        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(&root_str))
            .and_where(Expr::col(Events::CreatedAt).lte(until))
            .order_by(Events::Sequence, Order::Asc)
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let query = Query::select()
            .distinct()
            .column(Events::Root)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .to_string(SqliteQueryBuilder);

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
            .distinct()
            .column(Events::Domain)
            .from(Events::Table)
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let domains = rows.iter().map(|row| row.get("domain")).collect();

        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        let root_str = root.to_string();

        let query = Query::select()
            .expr(Expr::col(Events::Sequence).max())
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(&root_str))
            .to_string(SqliteQueryBuilder);

        let row = sqlx::query(&query).fetch_optional(&self.pool).await?;

        match row {
            Some(row) => {
                let max_seq: Option<i32> = row.get(0);
                Ok(max_seq.map(|s| s as u32 + 1).unwrap_or(0))
            }
            None => Ok(0),
        }
    }

    async fn get_by_correlation(
        &self,
        correlation_id: &str,
    ) -> Result<Vec<crate::proto::EventBook>> {
        use crate::proto::{Cover, Edition, EventBook, Uuid as ProtoUuid};
        use std::collections::HashMap;

        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        let query = Query::select()
            .columns([
                Events::Domain,
                Events::Edition,
                Events::Root,
                Events::EventData,
                Events::Sequence,
            ])
            .from(Events::Table)
            .and_where(Expr::col(Events::CorrelationId).eq(correlation_id))
            .order_by(Events::Domain, Order::Asc)
            .order_by(Events::Root, Order::Asc)
            .order_by(Events::Sequence, Order::Asc)
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let mut books_map: HashMap<(String, String, Uuid), Vec<EventPage>> = HashMap::new();

        for row in rows {
            let domain: String = row.get("domain");
            let edition: String = row.get("edition");
            let root_str: String = row.get("root");
            let event_data: Vec<u8> = row.get("event_data");

            let root = Uuid::parse_str(&root_str)?;
            let event = EventPage::decode(event_data.as_slice())?;

            books_map.entry((domain, edition, root)).or_default().push(event);
        }

        let books = books_map
            .into_iter()
            .map(|((domain, edition, root), pages)| EventBook {
                cover: Some(Cover {
                    domain,
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                    correlation_id: correlation_id.to_string(),
                    edition: Some(Edition { name: edition, divergences: vec![] }),
                }),
                pages,
                snapshot: None,
                snapshot_state: None,
            })
            .collect();

        Ok(books)
    }

    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32> {
        let query = Query::delete()
            .from_table(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .to_string(SqliteQueryBuilder);

        let result = sqlx::query(&query).execute(&self.pool).await?;
        Ok(result.rows_affected() as u32)
    }
}
