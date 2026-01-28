//! SQLite projector handlers for standalone mode E2E tests.
//!
//! Implements `ProjectorHandler` for web and accounting read models using
//! in-memory SQLite. These are test-only implementations that mirror the
//! production PostgreSQL projectors.

use async_trait::async_trait;
use prost::Message;
use sqlx::SqlitePool;
use tonic::Status;
use tracing::warn;

use angzarr::proto::{EventBook, Projection};
use angzarr::standalone::ProjectorHandler;
use common::proto as examples_proto;

fn extract_event_type(event: &prost_types::Any) -> &str {
    event.type_url.rsplit('/').next().unwrap_or(&event.type_url)
}

fn root_id_string(book: &EventBook) -> String {
    book.cover
        .as_ref()
        .and_then(|c| c.root.as_ref())
        .and_then(|r| uuid::Uuid::from_slice(&r.value).ok())
        .map(|u| u.to_string())
        .unwrap_or_default()
}

// ============================================================================
// Web Projector — order summary read model
// ============================================================================

/// SQLite-backed web projector for order summaries.
///
/// Subscribes to "order" domain events and maintains a `customer_orders` table.
pub struct WebProjector {
    pool: SqlitePool,
}

impl WebProjector {
    pub async fn new(pool: SqlitePool) -> Result<Self, sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS customer_orders (
                order_id TEXT PRIMARY KEY,
                customer_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'pending',
                subtotal_cents INTEGER NOT NULL DEFAULT 0,
                discount_cents INTEGER NOT NULL DEFAULT 0,
                total_cents INTEGER NOT NULL DEFAULT 0,
                loyalty_points_used INTEGER NOT NULL DEFAULT 0,
                loyalty_points_earned INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await?;
        Ok(Self { pool })
    }

    async fn process_event(&self, event: &prost_types::Any, order_id: &str) {
        let event_type = extract_event_type(event);
        let result = match event_type {
            "examples.OrderCreated" => self.handle_order_created(event, order_id).await,
            "examples.PaymentSubmitted" => self.handle_status_update(order_id, "paid").await,
            "examples.OrderCompleted" => self.handle_order_completed(event, order_id).await,
            "examples.OrderCancelled" => self.handle_status_update(order_id, "cancelled").await,
            "examples.LoyaltyDiscountApplied" => {
                self.handle_loyalty_discount(event, order_id).await
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            warn!(error = %e, %event_type, %order_id, "Web projector failed");
        }
    }

    async fn handle_order_created(
        &self,
        event: &prost_types::Any,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        let evt = examples_proto::OrderCreated::decode(event.value.as_slice())
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        sqlx::query(
            "INSERT OR REPLACE INTO customer_orders
             (order_id, customer_id, status, subtotal_cents, total_cents)
             VALUES (?, ?, 'pending', ?, ?)",
        )
        .bind(order_id)
        .bind(&evt.customer_id)
        .bind(evt.subtotal_cents as i64)
        .bind(evt.subtotal_cents as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn handle_status_update(&self, order_id: &str, status: &str) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE customer_orders SET status = ? WHERE order_id = ?")
            .bind(status)
            .bind(order_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn handle_order_completed(
        &self,
        event: &prost_types::Any,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        if let Ok(evt) = examples_proto::OrderCompleted::decode(event.value.as_slice()) {
            sqlx::query(
                "UPDATE customer_orders SET status = 'completed',
                 total_cents = ?, loyalty_points_earned = ? WHERE order_id = ?",
            )
            .bind(evt.final_total_cents as i64)
            .bind(evt.loyalty_points_earned)
            .bind(order_id)
            .execute(&self.pool)
            .await?;
        } else {
            self.handle_status_update(order_id, "completed").await?;
        }
        Ok(())
    }

    async fn handle_loyalty_discount(
        &self,
        event: &prost_types::Any,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        let evt = examples_proto::LoyaltyDiscountApplied::decode(event.value.as_slice())
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        sqlx::query(
            "UPDATE customer_orders SET
             discount_cents = ?, total_cents = subtotal_cents - ?,
             loyalty_points_used = ? WHERE order_id = ?",
        )
        .bind(evt.discount_cents as i64)
        .bind(evt.discount_cents as i64)
        .bind(evt.points_used)
        .bind(order_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl ProjectorHandler for WebProjector {
    async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
        let order_id = root_id_string(events);
        let domain = events
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        // Only process order domain events
        if domain != "order" {
            return Ok(Projection::default());
        }

        for page in &events.pages {
            if let Some(event) = &page.event {
                self.process_event(event, &order_id).await;
            }
        }
        Ok(Projection::default())
    }
}

// ============================================================================
// Accounting Projector — ledger + loyalty balance read model
// ============================================================================

/// SQLite-backed accounting projector for financial ledger and loyalty balance.
///
/// Subscribes to "order" and "customer" domain events.
pub struct AccountingProjector {
    pool: SqlitePool,
}

impl AccountingProjector {
    pub async fn new(pool: SqlitePool) -> Result<Self, sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS accounting_ledger (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                order_id TEXT NOT NULL,
                entry_type TEXT NOT NULL,
                amount_cents INTEGER NOT NULL,
                created_at TEXT NOT NULL DEFAULT (datetime('now'))
            )",
        )
        .execute(&pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS loyalty_balance (
                customer_id TEXT PRIMARY KEY,
                current_points INTEGER NOT NULL DEFAULT 0,
                lifetime_points INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await?;

        Ok(Self { pool })
    }

    async fn process_event(&self, event: &prost_types::Any, root_id: &str, domain: &str) {
        let event_type = extract_event_type(event);
        let result = match (domain, event_type) {
            ("order", "examples.OrderCreated") => self.handle_order_created(event, root_id).await,
            ("order", "examples.LoyaltyDiscountApplied") => {
                self.handle_discount(event, root_id).await
            }
            ("order", "examples.OrderCancelled") => {
                self.handle_order_cancelled(event, root_id).await
            }
            ("customer", "examples.LoyaltyPointsAdded") => {
                self.handle_points_added(event, root_id).await
            }
            ("customer", "examples.LoyaltyPointsRedeemed") => {
                self.handle_points_redeemed(event, root_id).await
            }
            _ => Ok(()),
        };
        if let Err(e) = result {
            warn!(error = %e, %event_type, %root_id, "Accounting projector failed");
        }
    }

    async fn handle_order_created(
        &self,
        event: &prost_types::Any,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        let evt = examples_proto::OrderCreated::decode(event.value.as_slice())
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;
        sqlx::query(
            "INSERT INTO accounting_ledger (order_id, entry_type, amount_cents)
             VALUES (?, 'revenue', ?)",
        )
        .bind(order_id)
        .bind(evt.subtotal_cents as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn handle_discount(
        &self,
        event: &prost_types::Any,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        let evt = examples_proto::LoyaltyDiscountApplied::decode(event.value.as_slice())
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        // Update revenue entry to reflect discounted total
        sqlx::query(
            "UPDATE accounting_ledger SET amount_cents = amount_cents - ?
             WHERE order_id = ? AND entry_type = 'revenue'",
        )
        .bind(evt.discount_cents as i64)
        .bind(order_id)
        .execute(&self.pool)
        .await?;

        // Add discount ledger entry
        sqlx::query(
            "INSERT INTO accounting_ledger (order_id, entry_type, amount_cents)
             VALUES (?, 'discount', ?)",
        )
        .bind(order_id)
        .bind(evt.discount_cents as i64)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn handle_order_cancelled(
        &self,
        _event: &prost_types::Any,
        order_id: &str,
    ) -> Result<(), sqlx::Error> {
        // Look up the revenue amount to create a matching refund
        let revenue: Option<(i64,)> = sqlx::query_as(
            "SELECT amount_cents FROM accounting_ledger
             WHERE order_id = ? AND entry_type = 'revenue'",
        )
        .bind(order_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some((amount,)) = revenue {
            sqlx::query(
                "INSERT INTO accounting_ledger (order_id, entry_type, amount_cents)
                 VALUES (?, 'refund', ?)",
            )
            .bind(order_id)
            .bind(-amount)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    async fn handle_points_added(
        &self,
        event: &prost_types::Any,
        customer_id: &str,
    ) -> Result<(), sqlx::Error> {
        let evt = examples_proto::LoyaltyPointsAdded::decode(event.value.as_slice())
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        // Upsert loyalty balance
        sqlx::query(
            "INSERT INTO loyalty_balance (customer_id, current_points, lifetime_points)
             VALUES (?, ?, ?)
             ON CONFLICT(customer_id) DO UPDATE SET
             current_points = excluded.current_points,
             lifetime_points = excluded.lifetime_points",
        )
        .bind(customer_id)
        .bind(evt.new_balance)
        .bind(evt.new_lifetime_points)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn handle_points_redeemed(
        &self,
        event: &prost_types::Any,
        customer_id: &str,
    ) -> Result<(), sqlx::Error> {
        let evt = examples_proto::LoyaltyPointsRedeemed::decode(event.value.as_slice())
            .map_err(|e| sqlx::Error::Protocol(e.to_string()))?;

        // Update loyalty balance
        sqlx::query(
            "INSERT INTO loyalty_balance (customer_id, current_points, lifetime_points)
             VALUES (?, ?, 0)
             ON CONFLICT(customer_id) DO UPDATE SET
             current_points = excluded.current_points",
        )
        .bind(customer_id)
        .bind(evt.new_balance)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[async_trait]
impl ProjectorHandler for AccountingProjector {
    async fn handle(&self, events: &EventBook) -> Result<Projection, Status> {
        let root_id = root_id_string(events);
        let domain = events
            .cover
            .as_ref()
            .map(|c| c.domain.as_str())
            .unwrap_or("");

        for page in &events.pages {
            if let Some(event) = &page.event {
                self.process_event(event, &root_id, domain).await;
            }
        }
        Ok(Projection::default())
    }
}

/// Create shared in-memory SQLite pool for projector tests.
///
/// Uses a named in-memory database with shared cache so multiple
/// connections (projector writer + test reader) see the same data.
pub async fn create_projector_pool(name: &str) -> Result<SqlitePool, sqlx::Error> {
    use sqlx::sqlite::SqlitePoolOptions;

    SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!("sqlite:file:{}?mode=memory&cache=shared", name))
        .await
}
