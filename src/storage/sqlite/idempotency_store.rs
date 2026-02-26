//! SQLite implementation of IdempotencyStore.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use uuid::Uuid;

use crate::storage::{IdempotencyRecord, IdempotencyStore, Result};

/// SQLite implementation of IdempotencyStore.
pub struct SqliteIdempotencyStore {
    pool: SqlitePool,
}

impl SqliteIdempotencyStore {
    /// Create a new SQLite idempotency store.
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IdempotencyStore for SqliteIdempotencyStore {
    async fn try_claim(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        external_id: &str,
        first_sequence: u32,
        last_sequence: u32,
    ) -> Result<Option<IdempotencyRecord>> {
        let root_str = root.to_string();
        let now = Utc::now().to_rfc3339();

        // Try to insert - SQLite will fail with UNIQUE constraint if already exists
        let insert_result = sqlx::query(
            r#"
            INSERT INTO fact_idempotency (domain, edition, root, external_id, first_sequence, last_sequence, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(domain)
        .bind(edition)
        .bind(&root_str)
        .bind(external_id)
        .bind(first_sequence as i64)
        .bind(last_sequence as i64)
        .bind(&now)
        .execute(&self.pool)
        .await;

        match insert_result {
            Ok(_) => {
                // Successfully claimed
                Ok(None)
            }
            Err(sqlx::Error::Database(db_err)) if db_err.is_unique_violation() => {
                // Already claimed - fetch and return existing record
                self.get(domain, edition, root, external_id).await
            }
            Err(e) => Err(e.into()),
        }
    }

    async fn get(
        &self,
        domain: &str,
        edition: &str,
        root: Uuid,
        external_id: &str,
    ) -> Result<Option<IdempotencyRecord>> {
        let root_str = root.to_string();

        let row: Option<(String, String, String, String, i64, i64, String)> = sqlx::query_as(
            r#"
            SELECT domain, edition, root, external_id, first_sequence, last_sequence, created_at
            FROM fact_idempotency
            WHERE domain = ? AND edition = ? AND root = ? AND external_id = ?
            "#,
        )
        .bind(domain)
        .bind(edition)
        .bind(&root_str)
        .bind(external_id)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some((domain, edition, root_str, external_id, first_seq, last_seq, created_at)) => {
                let root = Uuid::parse_str(&root_str)?;
                let created_at = DateTime::parse_from_rfc3339(&created_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                Ok(Some(IdempotencyRecord {
                    domain,
                    edition,
                    root,
                    external_id,
                    first_sequence: first_seq as u32,
                    last_sequence: last_seq as u32,
                    created_at,
                }))
            }
            None => Ok(None),
        }
    }
}
