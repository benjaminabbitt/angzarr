//! SQLite implementations of storage interfaces.

use async_trait::async_trait;
use prost::Message;
use sea_query::{ColumnDef, Expr, Index, OnConflict, Order, Query, SqliteQueryBuilder, Table};
use sqlx::{Acquire, Row, SqlitePool};
use uuid::Uuid;

use super::schema::{Events, Snapshots};
use super::{EventStore, Result, SnapshotStore};
use crate::proto::{EventPage, Snapshot};

/// SQLite implementation of EventStore.
pub struct SqliteEventStore {
    pool: SqlitePool,
}

impl SqliteEventStore {
    /// Create a new SQLite event store.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the database schema.
    pub async fn init(&self) -> Result<()> {
        // Create events table using sea-query
        let create_table = Table::create()
            .table(Events::Table)
            .if_not_exists()
            .col(ColumnDef::new(Events::Domain).text().not_null())
            .col(ColumnDef::new(Events::Root).text().not_null())
            .col(ColumnDef::new(Events::Sequence).integer().not_null())
            .col(ColumnDef::new(Events::CreatedAt).text().not_null())
            .col(ColumnDef::new(Events::EventData).binary().not_null())
            .col(
                ColumnDef::new(Events::CorrelationId)
                    .text()
                    .not_null()
                    .default(""),
            )
            .primary_key(
                Index::create()
                    .col(Events::Domain)
                    .col(Events::Root)
                    .col(Events::Sequence),
            )
            .to_string(SqliteQueryBuilder);

        sqlx::query(&create_table).execute(&self.pool).await?;

        // Create index for domain+root lookups
        let create_index = Index::create()
            .if_not_exists()
            .name("idx_events_domain_root")
            .table(Events::Table)
            .col(Events::Domain)
            .col(Events::Root)
            .to_string(SqliteQueryBuilder);

        sqlx::query(&create_index).execute(&self.pool).await?;

        // Create index for correlation_id lookups (process manager queries)
        let create_correlation_index = Index::create()
            .if_not_exists()
            .name("idx_events_correlation_id")
            .table(Events::Table)
            .col(Events::CorrelationId)
            .to_string(SqliteQueryBuilder);

        sqlx::query(&create_correlation_index)
            .execute(&self.pool)
            .await?;

        Ok(())
    }
}

#[async_trait]
impl EventStore for SqliteEventStore {
    async fn add(
        &self,
        domain: &str,
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
                .and_where(Expr::col(Events::Domain).eq(domain))
                .and_where(Expr::col(Events::Root).eq(&root_str))
                .to_string(SqliteQueryBuilder);

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
            let sequence =
                super::helpers::resolve_sequence(&event, base_sequence, &mut auto_sequence)?;
            let created_at = super::helpers::parse_timestamp(&event)?;

            let query = Query::insert()
                .into_table(Events::Table)
                .columns([
                    Events::Domain,
                    Events::Root,
                    Events::Sequence,
                    Events::CreatedAt,
                    Events::EventData,
                    Events::CorrelationId,
                ])
                .values_panic([
                    domain.into(),
                    root_str.clone().into(),
                    sequence.into(),
                    created_at.into(),
                    event_data.into(),
                    correlation_id.into(),
                ])
                .to_string(SqliteQueryBuilder);

            sqlx::query(&query).execute(&mut *tx).await?;
        }

        // Commit the transaction
        tx.commit().await?;

        Ok(())
    }

    async fn get(&self, domain: &str, root: Uuid) -> Result<Vec<EventPage>> {
        self.get_from(domain, root, 0).await
    }

    async fn get_from(&self, domain: &str, root: Uuid, from: u32) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
            .and_where(Expr::col(Events::Domain).eq(domain))
            .and_where(Expr::col(Events::Root).eq(&root_str))
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

    async fn get_from_to(
        &self,
        domain: &str,
        root: Uuid,
        from: u32,
        to: u32,
    ) -> Result<Vec<EventPage>> {
        let root_str = root.to_string();

        let query = Query::select()
            .column(Events::EventData)
            .from(Events::Table)
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

    async fn list_roots(&self, domain: &str) -> Result<Vec<Uuid>> {
        let query = Query::select()
            .distinct()
            .column(Events::Root)
            .from(Events::Table)
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

    async fn get_next_sequence(&self, domain: &str, root: Uuid) -> Result<u32> {
        let root_str = root.to_string();

        let query = Query::select()
            .expr(Expr::col(Events::Sequence).max())
            .from(Events::Table)
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
        use crate::proto::{Cover, EventBook, Uuid as ProtoUuid};
        use std::collections::HashMap;

        if correlation_id.is_empty() {
            return Ok(vec![]);
        }

        // Query all events with this correlation_id
        let query = Query::select()
            .columns([
                Events::Domain,
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

        // Group events by (domain, root)
        let mut books_map: HashMap<(String, Uuid), Vec<EventPage>> = HashMap::new();

        for row in rows {
            let domain: String = row.get("domain");
            let root_str: String = row.get("root");
            let event_data: Vec<u8> = row.get("event_data");

            let root = Uuid::parse_str(&root_str)?;
            let event = EventPage::decode(event_data.as_slice())?;

            books_map
                .entry((domain, root))
                .or_default()
                .push(event);
        }

        // Convert to EventBooks
        let books = books_map
            .into_iter()
            .map(|((domain, root), pages)| EventBook {
                cover: Some(Cover {
                    domain,
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                    correlation_id: correlation_id.to_string(),
                }),
                pages,
                snapshot: None,
                snapshot_state: None,
            })
            .collect();

        Ok(books)
    }
}

/// SQLite implementation of SnapshotStore.
pub struct SqliteSnapshotStore {
    pool: SqlitePool,
}

impl SqliteSnapshotStore {
    /// Create a new SQLite snapshot store.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Initialize the database schema.
    pub async fn init(&self) -> Result<()> {
        let create_table = Table::create()
            .table(Snapshots::Table)
            .if_not_exists()
            .col(ColumnDef::new(Snapshots::Domain).text().not_null())
            .col(ColumnDef::new(Snapshots::Root).text().not_null())
            .col(ColumnDef::new(Snapshots::Sequence).integer().not_null())
            .col(ColumnDef::new(Snapshots::StateData).binary().not_null())
            .col(ColumnDef::new(Snapshots::CreatedAt).text().not_null())
            .primary_key(Index::create().col(Snapshots::Domain).col(Snapshots::Root))
            .to_string(SqliteQueryBuilder);

        sqlx::query(&create_table).execute(&self.pool).await?;
        Ok(())
    }
}

#[async_trait]
impl SnapshotStore for SqliteSnapshotStore {
    async fn get(&self, domain: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let root_str = root.to_string();

        let query = Query::select()
            .column(Snapshots::StateData)
            .column(Snapshots::Sequence)
            .from(Snapshots::Table)
            .and_where(Expr::col(Snapshots::Domain).eq(domain))
            .and_where(Expr::col(Snapshots::Root).eq(&root_str))
            .to_string(SqliteQueryBuilder);

        let row = sqlx::query(&query).fetch_optional(&self.pool).await?;

        match row {
            Some(row) => {
                let state_data: Vec<u8> = row.get("state_data");
                let snapshot = Snapshot::decode(state_data.as_slice())?;
                Ok(Some(snapshot))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, domain: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let root_str = root.to_string();
        let state_data = snapshot.encode_to_vec();
        let sequence = snapshot.sequence;
        let created_at = chrono::Utc::now().to_rfc3339();

        let query = Query::insert()
            .into_table(Snapshots::Table)
            .columns([
                Snapshots::Domain,
                Snapshots::Root,
                Snapshots::Sequence,
                Snapshots::StateData,
                Snapshots::CreatedAt,
            ])
            .values_panic([
                domain.into(),
                root_str.into(),
                sequence.into(),
                state_data.into(),
                created_at.into(),
            ])
            .on_conflict(
                OnConflict::columns([Snapshots::Domain, Snapshots::Root])
                    .update_columns([
                        Snapshots::Sequence,
                        Snapshots::StateData,
                        Snapshots::CreatedAt,
                    ])
                    .to_owned(),
            )
            .to_string(SqliteQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }

    async fn delete(&self, domain: &str, root: Uuid) -> Result<()> {
        let root_str = root.to_string();

        let query = Query::delete()
            .from_table(Snapshots::Table)
            .and_where(Expr::col(Snapshots::Domain).eq(domain))
            .and_where(Expr::col(Snapshots::Root).eq(&root_str))
            .to_string(SqliteQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }
}
