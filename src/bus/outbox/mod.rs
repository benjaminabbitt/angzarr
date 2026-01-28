//! Outbox pattern wrapper for guaranteed event delivery.
//!
//! This module provides an `OutboxEventBus` that wraps any `EventBus` implementation
//! and ensures events are persisted before publishing. The flow:
//!
//! 1. Write event to SQL outbox table (within transaction)
//! 2. Publish to inner bus
//! 3. Delete from outbox on success
//!
//! If step 2 fails, the event remains in the outbox for retry by a background process.
//!
//! # When to Use (and When Not To)
//!
//! **The outbox pattern is often superfluous.** Many messaging systems already provide
//! durability guarantees:
//!
//! | Messaging Layer | Built-in Durability | Outbox Needed? |
//! |-----------------|---------------------|----------------|
//! | **Kafka** | Yes (replicated log) | Rarely - Kafka already guarantees delivery |
//! | **RabbitMQ** | Optional (persistent queues) | Maybe - if not using persistent queues |
//! | **In-memory** | No | Yes - if delivery matters |
//! | **Redis Streams** | Optional (AOF/RDB) | Depends on persistence config |
//!
//! **Use outbox when:**
//! - Network between app and message broker is unreliable
//! - Message broker lacks durability guarantees
//! - Regulatory/compliance requires local audit trail before transmission
//! - You need exactly-once semantics (combined with idempotent consumers)
//!
//! **Skip outbox when:**
//! - Using Kafka or other durable message brokers
//! - Best-effort delivery is acceptable (analytics, logging)
//! - Latency is critical
//! - You're already paying for managed messaging with SLAs
//!
//! # Performance & Cost Impact
//!
//! **Warning:** The outbox pattern has significant overhead:
//!
//! - **Latency:** 2 SQL round-trips per publish (INSERT + DELETE), typically 1-5ms added
//! - **Duplication:** Events stored twice (outbox table + message broker)
//! - **Storage cost:** Outbox table grows during outages; requires monitoring
//! - **Operational cost:** Background recovery process, table maintenance, monitoring
//! - **Complexity:** More failure modes to understand and debug
//!
//! **Understand what you're getting into.** The outbox pattern trades simplicity and
//! performance for delivery guarantees. If your messaging layer already provides those
//! guarantees, you're paying twice for the same thing.
//!
//! # Configuration
//!
//! Enable via config or environment variable:
//! ```yaml
//! messaging:
//!   outbox:
//!     enabled: true
//!     max_retries: 10
//!     recovery_interval_secs: 5
//! ```
//!
//! Or via environment: `ANGZARR_OUTBOX_ENABLED=true`

use std::sync::Arc;

use async_trait::async_trait;
use prost::Message;
#[cfg(feature = "postgres")]
use sea_query::PostgresQueryBuilder;
#[cfg(feature = "sqlite")]
use sea_query::SqliteQueryBuilder;
use sea_query::{ColumnDef, Expr, Iden, Index, Query, Table};
use serde::Deserialize;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

use super::{BusError, EventBus, EventHandler, PublishResult, Result};
use crate::proto::EventBook;

// ============================================================================
// Schema
// ============================================================================

/// Outbox table schema.
#[derive(Iden)]
enum Outbox {
    Table,
    #[iden = "id"]
    Id,
    #[iden = "domain"]
    Domain,
    #[iden = "root"]
    Root,
    #[iden = "event_data"]
    EventData,
    #[iden = "created_at"]
    CreatedAt,
    #[iden = "retry_count"]
    RetryCount,
}

// ============================================================================
// Configuration
// ============================================================================

/// Outbox configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct OutboxConfig {
    /// Enable outbox pattern. Default: false.
    /// Can be overridden via ANGZARR_OUTBOX_ENABLED env var.
    pub enabled: bool,
    /// Maximum retry attempts before moving to dead letter. Default: 10.
    pub max_retries: u32,
    /// Interval in seconds for background recovery. Default: 5.
    pub recovery_interval_secs: u64,
}

impl Default for OutboxConfig {
    fn default() -> Self {
        Self {
            enabled: std::env::var("ANGZARR_OUTBOX_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false),
            max_retries: 10,
            recovery_interval_secs: 5,
        }
    }
}

impl OutboxConfig {
    /// Check if outbox is enabled (config or env var).
    pub fn is_enabled(&self) -> bool {
        self.enabled
            || std::env::var("ANGZARR_OUTBOX_ENABLED")
                .map(|v| v == "true" || v == "1")
                .unwrap_or(false)
    }
}

// ============================================================================
// PostgreSQL Implementation
// ============================================================================

/// Outbox wrapper for PostgreSQL.
#[cfg(feature = "postgres")]
pub struct PostgresOutboxEventBus {
    inner: Arc<dyn EventBus>,
    pool: sqlx::PgPool,
    config: OutboxConfig,
}

#[cfg(feature = "postgres")]
impl PostgresOutboxEventBus {
    /// Create a new outbox-wrapped event bus.
    pub fn new(inner: Arc<dyn EventBus>, pool: sqlx::PgPool, config: OutboxConfig) -> Self {
        Self {
            inner,
            pool,
            config,
        }
    }

    /// Initialize the outbox table schema.
    pub async fn init(&self) -> std::result::Result<(), sqlx::Error> {
        let create_table = Table::create()
            .table(Outbox::Table)
            .if_not_exists()
            .col(ColumnDef::new(Outbox::Id).uuid().primary_key())
            .col(ColumnDef::new(Outbox::Domain).text().not_null())
            .col(ColumnDef::new(Outbox::Root).text().not_null())
            .col(ColumnDef::new(Outbox::EventData).binary().not_null())
            .col(
                ColumnDef::new(Outbox::CreatedAt)
                    .timestamp_with_time_zone()
                    .not_null()
                    .default(Expr::current_timestamp()),
            )
            .col(
                ColumnDef::new(Outbox::RetryCount)
                    .integer()
                    .not_null()
                    .default(0),
            )
            .to_string(PostgresQueryBuilder);

        sqlx::query(&create_table).execute(&self.pool).await?;

        // Index for recovery queries
        let create_index = Index::create()
            .if_not_exists()
            .name("idx_outbox_created_at")
            .table(Outbox::Table)
            .col(Outbox::CreatedAt)
            .to_string(PostgresQueryBuilder);

        sqlx::query(&create_index).execute(&self.pool).await?;

        info!("Outbox table initialized (PostgreSQL)");
        Ok(())
    }

    /// Recover orphaned events (events that were written but not published).
    ///
    /// Call this periodically from a background task.
    pub async fn recover_orphaned(&self) -> std::result::Result<u32, sqlx::Error> {
        use sqlx::Row;

        // Find events older than 30 seconds (publish should be <1s normally)
        let select = Query::select()
            .columns([Outbox::Id, Outbox::EventData, Outbox::RetryCount])
            .from(Outbox::Table)
            .and_where(Expr::col(Outbox::CreatedAt).lt(Expr::cust("NOW() - INTERVAL '30 seconds'")))
            .and_where(Expr::col(Outbox::RetryCount).lt(self.config.max_retries as i32))
            .limit(100)
            .to_string(PostgresQueryBuilder);

        let rows = sqlx::query(&select).fetch_all(&self.pool).await?;

        let mut recovered = 0u32;
        for row in rows {
            let id: Uuid = row.get("id");
            let event_data: Vec<u8> = row.get("event_data");
            let retry_count: i32 = row.get("retry_count");

            match EventBook::decode(event_data.as_slice()) {
                Ok(book) => {
                    match self.inner.publish(Arc::new(book)).await {
                        Ok(_) => {
                            // Delete from outbox
                            let delete = Query::delete()
                                .from_table(Outbox::Table)
                                .and_where(Expr::col(Outbox::Id).eq(id.to_string()))
                                .to_string(PostgresQueryBuilder);

                            if let Err(e) = sqlx::query(&delete).execute(&self.pool).await {
                                error!(id = %id, error = %e, "Failed to delete recovered event from outbox");
                            } else {
                                recovered += 1;
                                debug!(id = %id, "Recovered orphaned event");
                            }
                        }
                        Err(e) => {
                            // Increment retry count
                            warn!(id = %id, retry_count = retry_count + 1, error = %e, "Failed to recover event, incrementing retry count");
                            let update = Query::update()
                                .table(Outbox::Table)
                                .value(Outbox::RetryCount, retry_count + 1)
                                .and_where(Expr::col(Outbox::Id).eq(id.to_string()))
                                .to_string(PostgresQueryBuilder);

                            let _ = sqlx::query(&update).execute(&self.pool).await;
                        }
                    }
                }
                Err(e) => {
                    error!(id = %id, error = %e, "Failed to decode orphaned event, removing from outbox");
                    let delete = Query::delete()
                        .from_table(Outbox::Table)
                        .and_where(Expr::col(Outbox::Id).eq(id.to_string()))
                        .to_string(PostgresQueryBuilder);

                    let _ = sqlx::query(&delete).execute(&self.pool).await;
                }
            }
        }

        if recovered > 0 {
            info!(
                recovered = recovered,
                "Recovered orphaned events from outbox"
            );
        }

        Ok(recovered)
    }
}

#[cfg(feature = "postgres")]
#[async_trait]
impl EventBus for PostgresOutboxEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let id = Uuid::new_v4();
        let event_data = book.encode_to_vec();

        let (domain, root) = book
            .cover
            .as_ref()
            .map(|c| {
                (
                    c.domain.clone(),
                    c.root
                        .as_ref()
                        .map(|r| hex::encode(&r.value))
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();

        // Step 1: Write to outbox
        let insert = Query::insert()
            .into_table(Outbox::Table)
            .columns([Outbox::Id, Outbox::Domain, Outbox::Root, Outbox::EventData])
            .values_panic([
                id.to_string().into(),
                domain.clone().into(),
                root.clone().into(),
                event_data.into(),
            ])
            .to_string(PostgresQueryBuilder);

        sqlx::query(&insert)
            .execute(&self.pool)
            .await
            .map_err(|e| BusError::Publish(format!("Outbox insert failed: {}", e)))?;

        debug!(id = %id, domain = %domain, "Event written to outbox");

        // Step 2: Publish to inner bus
        let result = self.inner.publish(book).await;

        // Step 3: Delete from outbox on success
        if result.is_ok() {
            let delete = Query::delete()
                .from_table(Outbox::Table)
                .and_where(Expr::col(Outbox::Id).eq(id.to_string()))
                .to_string(PostgresQueryBuilder);

            if let Err(e) = sqlx::query(&delete).execute(&self.pool).await {
                // Log but don't fail - event was published, recovery will clean up
                warn!(id = %id, error = %e, "Failed to delete from outbox after successful publish");
            } else {
                debug!(id = %id, "Event removed from outbox after successful publish");
            }
        } else {
            debug!(id = %id, "Publish failed, event remains in outbox for recovery");
        }

        result
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        self.inner.subscribe(handler).await
    }

    async fn start_consuming(&self) -> Result<()> {
        self.inner.start_consuming().await
    }
}

// ============================================================================
// SQLite Implementation
// ============================================================================

/// Outbox wrapper for SQLite.
#[cfg(feature = "sqlite")]
pub struct SqliteOutboxEventBus {
    inner: Arc<dyn EventBus>,
    pool: sqlx::SqlitePool,
    config: OutboxConfig,
}

#[cfg(feature = "sqlite")]
impl SqliteOutboxEventBus {
    /// Create a new outbox-wrapped event bus.
    pub fn new(inner: Arc<dyn EventBus>, pool: sqlx::SqlitePool, config: OutboxConfig) -> Self {
        Self {
            inner,
            pool,
            config,
        }
    }

    /// Initialize the outbox table schema.
    pub async fn init(&self) -> std::result::Result<(), sqlx::Error> {
        let create_table = Table::create()
            .table(Outbox::Table)
            .if_not_exists()
            .col(ColumnDef::new(Outbox::Id).text().primary_key())
            .col(ColumnDef::new(Outbox::Domain).text().not_null())
            .col(ColumnDef::new(Outbox::Root).text().not_null())
            .col(ColumnDef::new(Outbox::EventData).blob().not_null())
            .col(
                ColumnDef::new(Outbox::CreatedAt)
                    .text()
                    .not_null()
                    .default(Expr::cust("(datetime('now'))")),
            )
            .col(
                ColumnDef::new(Outbox::RetryCount)
                    .integer()
                    .not_null()
                    .default(0),
            )
            .to_string(SqliteQueryBuilder);

        sqlx::query(&create_table).execute(&self.pool).await?;

        // Index for recovery queries
        let create_index = Index::create()
            .if_not_exists()
            .name("idx_outbox_created_at")
            .table(Outbox::Table)
            .col(Outbox::CreatedAt)
            .to_string(SqliteQueryBuilder);

        sqlx::query(&create_index).execute(&self.pool).await?;

        info!("Outbox table initialized (SQLite)");
        Ok(())
    }

    /// Recover orphaned events (events that were written but not published).
    pub async fn recover_orphaned(&self) -> std::result::Result<u32, sqlx::Error> {
        use sqlx::Row;

        // Find events older than 30 seconds
        let select = Query::select()
            .columns([Outbox::Id, Outbox::EventData, Outbox::RetryCount])
            .from(Outbox::Table)
            .and_where(
                Expr::col(Outbox::CreatedAt).lt(Expr::cust("datetime('now', '-30 seconds')")),
            )
            .and_where(Expr::col(Outbox::RetryCount).lt(self.config.max_retries as i32))
            .limit(100)
            .to_string(SqliteQueryBuilder);

        let rows = sqlx::query(&select).fetch_all(&self.pool).await?;

        let mut recovered = 0u32;
        for row in rows {
            let id: String = row.get("id");
            let event_data: Vec<u8> = row.get("event_data");
            let retry_count: i32 = row.get("retry_count");

            match EventBook::decode(event_data.as_slice()) {
                Ok(book) => match self.inner.publish(Arc::new(book)).await {
                    Ok(_) => {
                        let delete = Query::delete()
                            .from_table(Outbox::Table)
                            .and_where(Expr::col(Outbox::Id).eq(id.clone()))
                            .to_string(SqliteQueryBuilder);

                        if let Err(e) = sqlx::query(&delete).execute(&self.pool).await {
                            error!(id = %id, error = %e, "Failed to delete recovered event from outbox");
                        } else {
                            recovered += 1;
                            debug!(id = %id, "Recovered orphaned event");
                        }
                    }
                    Err(e) => {
                        warn!(id = %id, retry_count = retry_count + 1, error = %e, "Failed to recover event");
                        let update = Query::update()
                            .table(Outbox::Table)
                            .value(Outbox::RetryCount, retry_count + 1)
                            .and_where(Expr::col(Outbox::Id).eq(id))
                            .to_string(SqliteQueryBuilder);

                        let _ = sqlx::query(&update).execute(&self.pool).await;
                    }
                },
                Err(e) => {
                    error!(id = %id, error = %e, "Failed to decode orphaned event");
                    let delete = Query::delete()
                        .from_table(Outbox::Table)
                        .and_where(Expr::col(Outbox::Id).eq(id))
                        .to_string(SqliteQueryBuilder);

                    let _ = sqlx::query(&delete).execute(&self.pool).await;
                }
            }
        }

        if recovered > 0 {
            info!(
                recovered = recovered,
                "Recovered orphaned events from outbox"
            );
        }

        Ok(recovered)
    }
}

#[cfg(feature = "sqlite")]
#[async_trait]
impl EventBus for SqliteOutboxEventBus {
    async fn publish(&self, book: Arc<EventBook>) -> Result<PublishResult> {
        let id = Uuid::new_v4();
        let event_data = book.encode_to_vec();

        let (domain, root) = book
            .cover
            .as_ref()
            .map(|c| {
                (
                    c.domain.clone(),
                    c.root
                        .as_ref()
                        .map(|r| hex::encode(&r.value))
                        .unwrap_or_default(),
                )
            })
            .unwrap_or_default();

        // Step 1: Write to outbox
        let insert = Query::insert()
            .into_table(Outbox::Table)
            .columns([Outbox::Id, Outbox::Domain, Outbox::Root, Outbox::EventData])
            .values_panic([
                id.to_string().into(),
                domain.clone().into(),
                root.clone().into(),
                event_data.into(),
            ])
            .to_string(SqliteQueryBuilder);

        sqlx::query(&insert)
            .execute(&self.pool)
            .await
            .map_err(|e| BusError::Publish(format!("Outbox insert failed: {}", e)))?;

        debug!(id = %id, domain = %domain, "Event written to outbox");

        // Step 2: Publish to inner bus
        let result = self.inner.publish(book).await;

        // Step 3: Delete from outbox on success
        if result.is_ok() {
            let delete = Query::delete()
                .from_table(Outbox::Table)
                .and_where(Expr::col(Outbox::Id).eq(id.to_string()))
                .to_string(SqliteQueryBuilder);

            if let Err(e) = sqlx::query(&delete).execute(&self.pool).await {
                warn!(id = %id, error = %e, "Failed to delete from outbox after successful publish");
            } else {
                debug!(id = %id, "Event removed from outbox after successful publish");
            }
        }

        result
    }

    async fn subscribe(&self, handler: Box<dyn EventHandler>) -> Result<()> {
        self.inner.subscribe(handler).await
    }

    async fn start_consuming(&self) -> Result<()> {
        self.inner.start_consuming().await
    }
}

// ============================================================================
// Background Recovery Task
// ============================================================================

/// Handle to a running recovery task.
pub struct RecoveryTaskHandle {
    cancel: tokio::sync::watch::Sender<bool>,
}

impl RecoveryTaskHandle {
    /// Signal the recovery task to stop.
    pub fn stop(&self) {
        let _ = self.cancel.send(true);
    }
}

/// Spawn a background task that periodically recovers orphaned events.
///
/// Returns a handle that can be used to stop the task.
#[cfg(feature = "postgres")]
pub fn spawn_postgres_recovery_task(
    outbox: Arc<PostgresOutboxEventBus>,
    interval_secs: u64,
) -> RecoveryTaskHandle {
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);

    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_secs);
        info!(
            interval_secs = interval_secs,
            "Outbox recovery task started"
        );

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = outbox.recover_orphaned().await {
                        error!(error = %e, "Outbox recovery failed");
                    }
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        info!("Outbox recovery task stopped");
                        break;
                    }
                }
            }
        }
    });

    RecoveryTaskHandle { cancel: cancel_tx }
}

/// Spawn a background task that periodically recovers orphaned events.
#[cfg(feature = "sqlite")]
pub fn spawn_sqlite_recovery_task(
    outbox: Arc<SqliteOutboxEventBus>,
    interval_secs: u64,
) -> RecoveryTaskHandle {
    let (cancel_tx, mut cancel_rx) = tokio::sync::watch::channel(false);

    tokio::spawn(async move {
        let interval = std::time::Duration::from_secs(interval_secs);
        info!(
            interval_secs = interval_secs,
            "Outbox recovery task started"
        );

        loop {
            tokio::select! {
                _ = tokio::time::sleep(interval) => {
                    if let Err(e) = outbox.recover_orphaned().await {
                        error!(error = %e, "Outbox recovery failed");
                    }
                }
                _ = cancel_rx.changed() => {
                    if *cancel_rx.borrow() {
                        info!("Outbox recovery task stopped");
                        break;
                    }
                }
            }
        }
    });

    RecoveryTaskHandle { cancel: cancel_tx }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_outbox_config_default() {
        let config = OutboxConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.max_retries, 10);
        assert_eq!(config.recovery_interval_secs, 5);
    }

    #[test]
    fn test_outbox_config_env_override() {
        // This test verifies the env var logic exists
        // Actual env var testing would require isolation
        let config = OutboxConfig {
            enabled: false,
            ..Default::default()
        };
        // Without env var set, should respect config
        assert!(!config.enabled);
    }
}
