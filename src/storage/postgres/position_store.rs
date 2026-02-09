//! PostgreSQL PositionStore implementation.

use async_trait::async_trait;
use sea_query::{Expr, OnConflict, PostgresQueryBuilder, Query};
use sqlx::{PgPool, Row};

use crate::storage::schema::Positions;
use crate::storage::{PositionStore, Result};

/// PostgreSQL implementation of PositionStore.
pub struct PostgresPositionStore {
    pool: PgPool,
}

impl PostgresPositionStore {
    /// Create a new PostgreSQL position store.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PositionStore for PostgresPositionStore {
    async fn get(
        &self,
        handler: &str,
        domain: &str,
        edition: &str,
        root: &[u8],
    ) -> Result<Option<u32>> {
        let query = Query::select()
            .column(Positions::Sequence)
            .from(Positions::Table)
            .and_where(Expr::col(Positions::Handler).eq(handler))
            .and_where(Expr::col(Positions::Edition).eq(edition))
            .and_where(Expr::col(Positions::Domain).eq(domain))
            .and_where(Expr::col(Positions::Root).eq(root))
            .to_string(PostgresQueryBuilder);

        let row = sqlx::query(&query).fetch_optional(&self.pool).await?;

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
    ) -> Result<()> {
        let updated_at = chrono::Utc::now().to_rfc3339();

        let query = Query::insert()
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
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;

        Ok(())
    }
}
