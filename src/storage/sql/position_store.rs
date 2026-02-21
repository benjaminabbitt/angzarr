//! Unified SQL PositionStore implementation.
//!
//! Uses a macro to generate implementations for each SQL backend,
//! eliminating code duplication while maintaining type safety.

use std::marker::PhantomData;

use super::SqlDatabase;

/// SQL-based implementation of PositionStore.
///
/// This generic implementation works with any SQL database that implements
/// the `SqlDatabase` trait (PostgreSQL, SQLite).
pub struct SqlPositionStore<DB: SqlDatabase> {
    pool: DB::Pool,
    _marker: PhantomData<DB>,
}

impl<DB: SqlDatabase> SqlPositionStore<DB> {
    /// Create a new SQL position store with the given pool.
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

/// Macro to implement PositionStore for a specific SQL backend.
///
/// This eliminates duplication between PostgreSQL and SQLite implementations
/// while maintaining full type safety.
macro_rules! impl_position_store {
    ($db_type:ty, $feature:literal) => {
        #[cfg(feature = $feature)]
        #[async_trait::async_trait]
        impl crate::storage::PositionStore for SqlPositionStore<$db_type> {
            async fn get(
                &self,
                handler: &str,
                domain: &str,
                edition: &str,
                root: &[u8],
            ) -> crate::storage::Result<Option<u32>> {
                use sea_query::{Expr, Query};
                use sqlx::Row;

                use crate::storage::schema::Positions;

                let stmt = Query::select()
                    .column(Positions::Sequence)
                    .from(Positions::Table)
                    .and_where(Expr::col(Positions::Handler).eq(handler))
                    .and_where(Expr::col(Positions::Edition).eq(edition))
                    .and_where(Expr::col(Positions::Domain).eq(domain))
                    .and_where(Expr::col(Positions::Root).eq(root))
                    .to_owned();

                let sql = <$db_type>::build_select(stmt);
                let row = sqlx::query(&sql).fetch_optional(&self.pool).await?;

                match row {
                    Some(row) => {
                        let sequence: i32 = row.get("sequence");
                        Ok(Some(sequence as u32))
                    }
                    None => Ok(None),
                }
            }

            async fn put(
                &self,
                handler: &str,
                domain: &str,
                edition: &str,
                root: &[u8],
                sequence: u32,
            ) -> crate::storage::Result<()> {
                use sea_query::{OnConflict, Query};

                use crate::storage::schema::Positions;

                let updated_at = chrono::Utc::now().to_rfc3339();

                let stmt = Query::insert()
                    .into_table(Positions::Table)
                    .columns([
                        Positions::Handler,
                        Positions::Edition,
                        Positions::Domain,
                        Positions::Root,
                        Positions::Sequence,
                        Positions::UpdatedAt,
                    ])
                    .values_panic([
                        handler.into(),
                        edition.into(),
                        domain.into(),
                        root.into(),
                        sequence.into(),
                        updated_at.into(),
                    ])
                    .on_conflict(
                        OnConflict::columns([
                            Positions::Handler,
                            Positions::Edition,
                            Positions::Domain,
                            Positions::Root,
                        ])
                        .update_columns([Positions::Sequence, Positions::UpdatedAt])
                        .to_owned(),
                    )
                    .to_owned();

                let sql = <$db_type>::build_insert(stmt);
                sqlx::query(&sql).execute(&self.pool).await?;

                Ok(())
            }
        }
    };
}

// Generate implementations for each SQL backend
impl_position_store!(super::postgres::Postgres, "postgres");
impl_position_store!(super::sqlite::Sqlite, "sqlite");
