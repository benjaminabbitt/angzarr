//! PostgreSQL EventStore implementation.
//!
//! Uses stored procedures for composite edition reads. The `get_edition_events`
//! stored procedure handles implicit divergence (deriving divergence point from
//! the first edition event).

use async_trait::async_trait;
use prost::Message;
use sea_query::{Expr, Order, PostgresQueryBuilder, Query};
use sqlx::{Acquire, PgPool, Row};
use uuid::Uuid;

use crate::orchestration::aggregate::DEFAULT_EDITION;
use crate::proto::EventPage;
use crate::storage::schema::Events;
use crate::storage::{EventStore, Result};

/// PostgreSQL implementation of EventStore.
pub struct PostgresEventStore {
    pool: PgPool,
}

impl PostgresEventStore {
    /// Create a new PostgreSQL event store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if edition is the main timeline.
    fn is_main_timeline(edition: &str) -> bool {
        edition.is_empty() || edition == DEFAULT_EDITION
    }

    /// Query events using the composite edition stored procedure.
    ///
    /// Calls `get_edition_events_from(domain, edition, root, from, explicit_divergence)`
    /// which handles implicit divergence (from first edition event) and main timeline
    /// merging.
    async fn composite_read(
        &self,
        domain: &str,
        edition: &str,
        root: &str,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        // Use stored procedure for composite read
        // The procedure handles: main timeline query if edition is 'angzarr',
        // or composite query (main + edition) with implicit divergence
        let query =
            format!("SELECT event_data FROM get_edition_events_from($1, $2, $3::uuid, $4, NULL)");

        let rows = sqlx::query(&query)
            .bind(domain)
            .bind(edition)
            .bind(root)
            .bind(from as i32)
            .fetch_all(&self.pool)
            .await?;

        let mut events = Vec::with_capacity(rows.len());
        for row in rows {
            let event_data: Vec<u8> = row.get("event_data");
            let event = EventPage::decode(event_data.as_slice())?;
            events.push(event);
        }

        Ok(events)
    }

    /// Simple query for main timeline events (no composite logic needed).
    async fn query_main_timeline(
        &self,
        domain: &str,
        root: &str,
        from: u32,
    ) -> Result<Vec<EventPage>> {
        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(DEFAULT_EDITION))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(root))
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
}

#[async_trait]
impl EventStore for PostgresEventStore {
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

        // Use a transaction to ensure atomicity
        let mut conn = self.pool.acquire().await?;
        let mut tx = conn.begin().await?;

        // Get the next sequence number once at the start of the transaction
        let base_sequence = {
            let query = Query::select()
                .expr(Expr::col(Events::Sequence).max())
                .from(Events::Table)
                .and_where(Expr::col(Events::Edition).eq(edition))
                .and_where(Expr::col(Events::Domain).eq(domain))
                .and_where(Expr::col(Events::Root).eq(&root_str))
                .to_string(PostgresQueryBuilder);

            let row = sqlx::query(&query).fetch_optional(&mut *tx).await?;

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

            sqlx::query(&query).execute(&mut *tx).await?;
        }

        // Commit the transaction
        tx.commit().await?;

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

        // Main timeline: simple query
        if Self::is_main_timeline(edition) {
            return self.query_main_timeline(domain, &root_str, from).await;
        }

        // Named edition: use stored procedure for composite read
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

        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(edition))
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

    async fn list_roots(&self, domain: &str, edition: &str) -> Result<Vec<Uuid>> {
        let query = Query::select()
            .distinct()
            .column(Events::Root)
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
            .distinct()
            .column(Events::Domain)
            .from(Events::Table)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        let domains = rows.iter().map(|row| row.get("domain")).collect();

        Ok(domains)
    }

    async fn get_next_sequence(&self, domain: &str, edition: &str, root: Uuid) -> Result<u32> {
        let root_str = root.to_string();

        // For non-default editions with implicit divergence, we need composite logic:
        // If the edition has no events yet, use the main timeline's max sequence
        if !Self::is_main_timeline(edition) {
            let edition_query = Query::select()
                .expr(Expr::col(Events::Sequence).max())
                .from(Events::Table)
                .and_where(Expr::col(Events::Edition).eq(edition))
                .and_where(Expr::col(Events::Domain).eq(domain))
                .and_where(Expr::col(Events::Root).eq(&root_str))
                .to_string(PostgresQueryBuilder);

            let edition_row = sqlx::query(&edition_query)
                .fetch_optional(&self.pool)
                .await?;

            if let Some(row) = edition_row {
                let max_seq: Option<i32> = row.get(0);
                if let Some(seq) = max_seq {
                    // Edition has events, use edition's max sequence
                    return Ok(seq as u32 + 1);
                }
            }

            // No edition events - fall through to check main timeline
        }

        // Query the target edition (or main timeline for fallback)
        let target_edition = if Self::is_main_timeline(edition) {
            edition
        } else {
            DEFAULT_EDITION
        };

        let query = Query::select()
            .expr(Expr::col(Events::Sequence).max())
            .from(Events::Table)
            .and_where(Expr::col(Events::Edition).eq(target_edition))
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(&root_str))
            .to_string(PostgresQueryBuilder);

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
        use crate::proto::{Cover, EventBook, Uuid as ProtoUuid};
        use std::collections::HashMap;

        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        // Query all events with this correlation_id
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
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::query(&query).fetch_all(&self.pool).await?;

        // Group events by (domain, edition, root)
        let mut books_map: HashMap<(String, String, Uuid), Vec<EventPage>> = HashMap::new();

        for row in rows {
            let domain: String = row.get("domain");
            let edition: String = row.get("edition");
            let root_str: String = row.get("root");
            let event_data: Vec<u8> = row.get("event_data");

            let root = Uuid::parse_str(&root_str)?;
            let event = EventPage::decode(event_data.as_slice())?;

            books_map
                .entry((domain, edition, root))
                .or_default()
                .push(event);
        }

        // Convert to EventBooks
        let books = books_map
            .into_iter()
            .map(|((domain, edition, root), pages)| EventBook {
                cover: Some(Cover {
                    domain,
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                    correlation_id: correlation_id.to_string(),
                    edition: Some(edition),
                }),
                pages,
                snapshot: None,
            })
            .collect();

        Ok(books)
    }

    async fn delete_edition_events(&self, domain: &str, edition: &str) -> Result<u32> {
        // The stored procedure handles main timeline protection
        let row = sqlx::query("SELECT delete_edition_events($1, $2)")
            .bind(edition)
            .bind(domain)
            .fetch_one(&self.pool)
            .await?;

        let count: i32 = row.get(0);
        Ok(count as u32)
    }
}
