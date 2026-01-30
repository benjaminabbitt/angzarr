//! PostgreSQL SnapshotStore implementation.

use async_trait::async_trait;
use prost::Message;
use sea_query::{Expr, OnConflict, PostgresQueryBuilder, Query};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::storage::schema::Snapshots;
use crate::storage::{Result, SnapshotStore};
use crate::proto::Snapshot;

/// PostgreSQL implementation of SnapshotStore.
pub struct PostgresSnapshotStore {
    pool: PgPool,
}

impl PostgresSnapshotStore {
    /// Create a new PostgreSQL snapshot store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SnapshotStore for PostgresSnapshotStore {
    async fn get(&self, domain: &str, edition: &str, root: Uuid) -> Result<Option<Snapshot>> {
        let root_str = root.to_string();

        let query = Query::select()
            .column(Snapshots::StateData)
            .column(Snapshots::Sequence)
            .from(Snapshots::Table)
            .and_where(Expr::col(Snapshots::Edition).eq(edition))
            .and_where(Expr::col(Snapshots::Domain).eq(domain))
            .and_where(Expr::col(Snapshots::Root).eq(&root_str))
            .to_string(PostgresQueryBuilder);

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

    async fn put(&self, domain: &str, edition: &str, root: Uuid, snapshot: Snapshot) -> Result<()> {
        let root_str = root.to_string();
        let state_data = snapshot.encode_to_vec();
        let sequence = snapshot.sequence;
        let created_at = chrono::Utc::now().to_rfc3339();

        let query = Query::insert()
            .into_table(Snapshots::Table)
            .columns([
                Snapshots::Edition,
                Snapshots::Domain,
                Snapshots::Root,
                Snapshots::Sequence,
                Snapshots::StateData,
                Snapshots::CreatedAt,
            ])
            .values_panic([
                edition.into(),
                domain.into(),
                root_str.into(),
                sequence.into(),
                state_data.into(),
                created_at.into(),
            ])
            .on_conflict(
                OnConflict::columns([Snapshots::Edition, Snapshots::Domain, Snapshots::Root])
                    .update_columns([
                        Snapshots::Sequence,
                        Snapshots::StateData,
                        Snapshots::CreatedAt,
                    ])
                    .to_owned(),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }

    async fn delete(&self, domain: &str, edition: &str, root: Uuid) -> Result<()> {
        let root_str = root.to_string();

        let query = Query::delete()
            .from_table(Snapshots::Table)
            .and_where(Expr::col(Snapshots::Edition).eq(edition))
            .and_where(Expr::col(Snapshots::Domain).eq(domain))
            .and_where(Expr::col(Snapshots::Root).eq(&root_str))
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }
}
