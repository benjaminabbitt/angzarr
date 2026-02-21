//! Unified SQL SnapshotStore implementation.
//!
//! Uses a macro to generate implementations for each SQL backend,
//! eliminating code duplication while maintaining type safety.

use std::marker::PhantomData;

use super::SqlDatabase;

/// SQL-based implementation of SnapshotStore.
///
/// This generic implementation works with any SQL database that implements
/// the `SqlDatabase` trait (PostgreSQL, SQLite).
pub struct SqlSnapshotStore<DB: SqlDatabase> {
    pool: DB::Pool,
    _marker: PhantomData<DB>,
}

impl<DB: SqlDatabase> SqlSnapshotStore<DB> {
    /// Create a new SQL snapshot store with the given pool.
    pub fn new(pool: DB::Pool) -> Self {
        Self {
            pool,
            _marker: PhantomData,
        }
    }

    /// Get the underlying pool.
    pub fn pool(&self) -> &DB::Pool {
        &self.pool
    }
}

/// Macro to implement SnapshotStore for a specific SQL backend.
///
/// The `$supports_get_at_seq` parameter controls whether the backend
/// supports historical snapshot queries (true for PostgreSQL, false for SQLite
/// which stores only the latest snapshot per aggregate).
macro_rules! impl_snapshot_store {
    ($db_type:ty, $feature:literal, supports_get_at_seq: $supports_get_at_seq:literal) => {
        #[cfg(feature = $feature)]
        #[async_trait::async_trait]
        impl crate::storage::SnapshotStore for SqlSnapshotStore<$db_type> {
            async fn get(
                &self,
                domain: &str,
                edition: &str,
                root: uuid::Uuid,
            ) -> crate::storage::Result<Option<crate::proto::Snapshot>> {
                use prost::Message;
                use sea_query::{Expr, Query};
                use sqlx::Row;

                use crate::storage::schema::Snapshots;

                let root_str = root.to_string();

                let stmt = Query::select()
                    .column(Snapshots::StateData)
                    .column(Snapshots::Sequence)
                    .from(Snapshots::Table)
                    .and_where(Expr::col(Snapshots::Edition).eq(edition))
                    .and_where(Expr::col(Snapshots::Domain).eq(domain))
                    .and_where(Expr::col(Snapshots::Root).eq(&root_str))
                    .to_owned();

                let sql = <$db_type>::build_select(stmt);
                let row = sqlx::query(&sql).fetch_optional(&self.pool).await?;

                match row {
                    Some(row) => {
                        let state_data: Vec<u8> = row.get("state_data");
                        let snapshot = crate::proto::Snapshot::decode(state_data.as_slice())?;
                        Ok(Some(snapshot))
                    }
                    None => Ok(None),
                }
            }

            async fn get_at_seq(
                &self,
                domain: &str,
                edition: &str,
                root: uuid::Uuid,
                seq: u32,
            ) -> crate::storage::Result<Option<crate::proto::Snapshot>> {
                // PostgreSQL supports historical snapshots, SQLite stores only latest
                if $supports_get_at_seq {
                    use prost::Message;
                    use sea_query::{Expr, Query};
                    use sqlx::Row;

                    use crate::storage::schema::Snapshots;

                    let root_str = root.to_string();

                    let stmt = Query::select()
                        .column(Snapshots::StateData)
                        .column(Snapshots::Sequence)
                        .from(Snapshots::Table)
                        .and_where(Expr::col(Snapshots::Edition).eq(edition))
                        .and_where(Expr::col(Snapshots::Domain).eq(domain))
                        .and_where(Expr::col(Snapshots::Root).eq(&root_str))
                        .and_where(Expr::col(Snapshots::Sequence).lte(seq))
                        .order_by(Snapshots::Sequence, sea_query::Order::Desc)
                        .limit(1)
                        .to_owned();

                    let sql = <$db_type>::build_select(stmt);
                    let row = sqlx::query(&sql).fetch_optional(&self.pool).await?;

                    match row {
                        Some(row) => {
                            let state_data: Vec<u8> = row.get("state_data");
                            let snapshot = crate::proto::Snapshot::decode(state_data.as_slice())?;
                            Ok(Some(snapshot))
                        }
                        None => Ok(None),
                    }
                } else {
                    // SQLite stores single snapshot per aggregate.
                    // Return it if it exists (sequence checking is simplified).
                    let _ = seq; // Suppress unused warning
                    self.get(domain, edition, root).await
                }
            }

            async fn put(
                &self,
                domain: &str,
                edition: &str,
                root: uuid::Uuid,
                snapshot: crate::proto::Snapshot,
            ) -> crate::storage::Result<()> {
                use prost::Message;
                use sea_query::{OnConflict, Query};

                use crate::storage::schema::Snapshots;

                let root_str = root.to_string();
                let state_data = snapshot.encode_to_vec();
                let sequence = snapshot.sequence;
                let created_at = chrono::Utc::now().to_rfc3339();

                let stmt = Query::insert()
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
                        OnConflict::columns([
                            Snapshots::Edition,
                            Snapshots::Domain,
                            Snapshots::Root,
                        ])
                        .update_columns([
                            Snapshots::Sequence,
                            Snapshots::StateData,
                            Snapshots::CreatedAt,
                        ])
                        .to_owned(),
                    )
                    .to_owned();

                let sql = <$db_type>::build_insert(stmt);
                sqlx::query(&sql).execute(&self.pool).await?;

                Ok(())
            }

            async fn delete(
                &self,
                domain: &str,
                edition: &str,
                root: uuid::Uuid,
            ) -> crate::storage::Result<()> {
                use sea_query::{Expr, Query};

                use crate::storage::schema::Snapshots;

                let root_str = root.to_string();

                let stmt = Query::delete()
                    .from_table(Snapshots::Table)
                    .and_where(Expr::col(Snapshots::Edition).eq(edition))
                    .and_where(Expr::col(Snapshots::Domain).eq(domain))
                    .and_where(Expr::col(Snapshots::Root).eq(&root_str))
                    .to_owned();

                let sql = <$db_type>::build_delete(stmt);
                sqlx::query(&sql).execute(&self.pool).await?;

                Ok(())
            }
        }
    };
}

// Generate implementations for each SQL backend
impl_snapshot_store!(super::postgres::Postgres, "postgres", supports_get_at_seq: true);
impl_snapshot_store!(super::sqlite::Sqlite, "sqlite", supports_get_at_seq: false);
