//! SQLite implementations of storage interfaces.

use async_trait::async_trait;
use prost::Message;
use sea_query::{Expr, OnConflict, Order, Query, SqliteQueryBuilder};
use sqlx::{Acquire, Row, SqlitePool};
use uuid::Uuid;

use crate::interfaces::event_store::{EventStore, Result, StorageError};
use crate::interfaces::snapshot_store::SnapshotStore;
use crate::proto::{EventPage, Snapshot};

use super::schema::{Events, Snapshots, CREATE_EVENTS_TABLE, CREATE_SNAPSHOTS_TABLE};

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
        sqlx::query(CREATE_EVENTS_TABLE).execute(&self.pool).await?;
        Ok(())
    }
}

#[async_trait]
impl EventStore for SqliteEventStore {
    async fn add(&self, domain: &str, root: Uuid, events: Vec<EventPage>) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }

        let root_str = root.to_string();

        // Use a transaction to ensure atomicity
        let mut conn = self.pool.acquire().await?;
        let mut tx = conn.begin().await?;

        // Get the next sequence number once at the start of the transaction
        // The transaction provides isolation from concurrent writes
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
                    let max_seq: Option<i64> = row.get(0);
                    max_seq.map(|s| s as u32 + 1).unwrap_or(0)
                }
                None => 0,
            }
        };

        let mut auto_sequence = base_sequence;

        for event in events {
            let event_data = event.encode_to_vec();

            // Determine sequence number
            let sequence = match &event.sequence {
                Some(crate::proto::event_page::Sequence::Num(n)) => {
                    // Validate explicit sequence numbers
                    if *n < base_sequence {
                        return Err(StorageError::SequenceConflict {
                            expected: base_sequence,
                            actual: *n,
                        });
                    }
                    *n
                }
                Some(crate::proto::event_page::Sequence::Force(_)) | None => {
                    let seq = auto_sequence;
                    auto_sequence += 1;
                    seq
                }
            };

            let synchronous = if event.synchronous { 1i32 } else { 0i32 };
            let created_at = event
                .created_at
                .as_ref()
                .map(|ts| {
                    chrono::DateTime::from_timestamp(ts.seconds, ts.nanos as u32).ok_or_else(|| {
                        StorageError::InvalidTimestamp {
                            seconds: ts.seconds,
                            nanos: ts.nanos,
                        }
                    })
                })
                .transpose()?
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

            let query = Query::insert()
                .into_table(Events::Table)
                .columns([
                    Events::Domain,
                    Events::Root,
                    Events::Sequence,
                    Events::CreatedAt,
                    Events::EventData,
                    Events::Synchronous,
                ])
                .values_panic([
                    domain.into(),
                    root_str.clone().into(),
                    sequence.into(),
                    created_at.into(),
                    event_data.into(),
                    synchronous.into(),
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
                let max_seq: Option<i64> = row.get(0);
                Ok(max_seq.map(|s| s as u32 + 1).unwrap_or(0))
            }
            None => Ok(0),
        }
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
        sqlx::query(CREATE_SNAPSHOTS_TABLE)
            .execute(&self.pool)
            .await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use prost_types::Timestamp;

    async fn test_pool() -> SqlitePool {
        SqlitePool::connect("sqlite::memory:").await.unwrap()
    }

    fn test_event(sequence: u32, event_type: &str) -> EventPage {
        EventPage {
            sequence: Some(crate::proto::event_page::Sequence::Num(sequence)),
            created_at: Some(Timestamp {
                seconds: 1704067200 + sequence as i64,
                nanos: 0,
            }),
            event: Some(prost_types::Any {
                type_url: format!("type.googleapis.com/{}", event_type),
                value: vec![1, 2, 3, sequence as u8],
            }),
            synchronous: false,
        }
    }

    fn test_snapshot(sequence: u32) -> Snapshot {
        Snapshot {
            sequence,
            state: Some(prost_types::Any {
                type_url: "type.googleapis.com/TestState".to_string(),
                value: vec![10, 20, 30],
            }),
        }
    }

    mod event_store {
        use super::*;

        #[tokio::test]
        async fn test_add_get_events() {
            let pool = test_pool().await;
            let store = SqliteEventStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();
            let events = vec![test_event(0, "TestCreated"), test_event(1, "TestUpdated")];

            store.add(domain, root, events).await.unwrap();

            let retrieved = store.get(domain, root).await.unwrap();
            assert_eq!(retrieved.len(), 2);
            assert_eq!(
                retrieved[0].sequence,
                Some(crate::proto::event_page::Sequence::Num(0))
            );
            assert_eq!(
                retrieved[1].sequence,
                Some(crate::proto::event_page::Sequence::Num(1))
            );
        }

        #[tokio::test]
        async fn test_get_from_sequence() {
            let pool = test_pool().await;
            let store = SqliteEventStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();
            let events = vec![
                test_event(0, "Event0"),
                test_event(1, "Event1"),
                test_event(2, "Event2"),
                test_event(3, "Event3"),
            ];

            store.add(domain, root, events).await.unwrap();

            let from_2 = store.get_from(domain, root, 2).await.unwrap();
            assert_eq!(from_2.len(), 2);
            assert_eq!(
                from_2[0].sequence,
                Some(crate::proto::event_page::Sequence::Num(2))
            );
            assert_eq!(
                from_2[1].sequence,
                Some(crate::proto::event_page::Sequence::Num(3))
            );
        }

        #[tokio::test]
        async fn test_get_from_to_range() {
            let pool = test_pool().await;
            let store = SqliteEventStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();
            let events = vec![
                test_event(0, "Event0"),
                test_event(1, "Event1"),
                test_event(2, "Event2"),
                test_event(3, "Event3"),
                test_event(4, "Event4"),
            ];

            store.add(domain, root, events).await.unwrap();

            let range = store.get_from_to(domain, root, 1, 4).await.unwrap();
            assert_eq!(range.len(), 3);
            assert_eq!(
                range[0].sequence,
                Some(crate::proto::event_page::Sequence::Num(1))
            );
            assert_eq!(
                range[2].sequence,
                Some(crate::proto::event_page::Sequence::Num(3))
            );
        }

        #[tokio::test]
        async fn test_list_roots() {
            let pool = test_pool().await;
            let store = SqliteEventStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root1 = Uuid::new_v4();
            let root2 = Uuid::new_v4();
            let root3 = Uuid::new_v4();

            store
                .add(domain, root1, vec![test_event(0, "Event")])
                .await
                .unwrap();
            store
                .add(domain, root2, vec![test_event(0, "Event")])
                .await
                .unwrap();
            store
                .add("other_domain", root3, vec![test_event(0, "Event")])
                .await
                .unwrap();

            let roots = store.list_roots(domain).await.unwrap();
            assert_eq!(roots.len(), 2);
            assert!(roots.contains(&root1));
            assert!(roots.contains(&root2));
            assert!(!roots.contains(&root3));
        }

        #[tokio::test]
        async fn test_get_next_sequence() {
            let pool = test_pool().await;
            let store = SqliteEventStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();

            let seq = store.get_next_sequence(domain, root).await.unwrap();
            assert_eq!(seq, 0);

            store
                .add(
                    domain,
                    root,
                    vec![test_event(0, "Event0"), test_event(1, "Event1")],
                )
                .await
                .unwrap();

            let seq = store.get_next_sequence(domain, root).await.unwrap();
            assert_eq!(seq, 2);
        }

        #[tokio::test]
        async fn test_get_empty_returns_empty_vec() {
            let pool = test_pool().await;
            let store = SqliteEventStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();

            let events = store.get(domain, root).await.unwrap();
            assert!(events.is_empty());
        }

        #[tokio::test]
        async fn test_events_isolated_by_domain() {
            let pool = test_pool().await;
            let store = SqliteEventStore::new(pool);
            store.init().await.unwrap();

            let root = Uuid::new_v4();

            store
                .add("domain_a", root, vec![test_event(0, "EventA")])
                .await
                .unwrap();
            store
                .add("domain_b", root, vec![test_event(0, "EventB")])
                .await
                .unwrap();

            let events_a = store.get("domain_a", root).await.unwrap();
            let events_b = store.get("domain_b", root).await.unwrap();

            assert_eq!(events_a.len(), 1);
            assert_eq!(events_b.len(), 1);
            assert_ne!(events_a[0].event, events_b[0].event);
        }
    }

    mod snapshot_store {
        use super::*;

        #[tokio::test]
        async fn test_put_get_snapshot() {
            let pool = test_pool().await;
            let store = SqliteSnapshotStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();
            let snapshot = test_snapshot(5);

            store.put(domain, root, snapshot.clone()).await.unwrap();

            let retrieved = store.get(domain, root).await.unwrap();
            assert!(retrieved.is_some());
            let retrieved = retrieved.unwrap();
            assert_eq!(retrieved.sequence, 5);
        }

        #[tokio::test]
        async fn test_get_nonexistent_returns_none() {
            let pool = test_pool().await;
            let store = SqliteSnapshotStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();

            let retrieved = store.get(domain, root).await.unwrap();
            assert!(retrieved.is_none());
        }

        #[tokio::test]
        async fn test_put_overwrites_existing() {
            let pool = test_pool().await;
            let store = SqliteSnapshotStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();

            store.put(domain, root, test_snapshot(5)).await.unwrap();
            store.put(domain, root, test_snapshot(10)).await.unwrap();

            let retrieved = store.get(domain, root).await.unwrap().unwrap();
            assert_eq!(retrieved.sequence, 10);
        }

        #[tokio::test]
        async fn test_delete_snapshot() {
            let pool = test_pool().await;
            let store = SqliteSnapshotStore::new(pool);
            store.init().await.unwrap();

            let domain = "test_domain";
            let root = Uuid::new_v4();

            store.put(domain, root, test_snapshot(5)).await.unwrap();
            assert!(store.get(domain, root).await.unwrap().is_some());

            store.delete(domain, root).await.unwrap();
            assert!(store.get(domain, root).await.unwrap().is_none());
        }

        #[tokio::test]
        async fn test_snapshots_isolated_by_domain() {
            let pool = test_pool().await;
            let store = SqliteSnapshotStore::new(pool);
            store.init().await.unwrap();

            let root = Uuid::new_v4();

            store.put("domain_a", root, test_snapshot(5)).await.unwrap();
            store
                .put("domain_b", root, test_snapshot(10))
                .await
                .unwrap();

            let snap_a = store.get("domain_a", root).await.unwrap().unwrap();
            let snap_b = store.get("domain_b", root).await.unwrap().unwrap();

            assert_eq!(snap_a.sequence, 5);
            assert_eq!(snap_b.sequence, 10);
        }
    }
}
