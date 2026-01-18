//! Accounting Projector - tracks revenue, discounts, and loyalty points.
//!
//! Listens to order and customer events and maintains financial ledger entries.

mod schema;

use prost::Message;
use sea_query::{ColumnDef, Expr, Index, OnConflict, PostgresQueryBuilder, Query, Table};
use sqlx::{PgPool, Row};
use tracing::{debug, error};
use uuid::Uuid;

use angzarr::proto::{EventBook, Projection};
use common::proto::{
    LoyaltyDiscountApplied, LoyaltyPointsAdded, LoyaltyPointsRedeemed, OrderCancelled,
    OrderCompleted, OrderCreated,
};

use schema::{AccountingLedger, LoyaltyBalance};

/// Errors that can occur during projection.
#[derive(Debug, thiserror::Error)]
pub enum ProjectorError {
    #[error("Storage error: {0}")]
    Storage(String),
}

pub type Result<T> = std::result::Result<T, ProjectorError>;

pub const PROJECTOR_NAME: &str = "accounting";

/// Entry types for the accounting ledger.
#[derive(Debug, Clone, Copy)]
pub enum LedgerEntryType {
    Revenue,
    Discount,
    Refund,
    LoyaltyEarned,
    LoyaltyRedeemed,
}

impl LedgerEntryType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LedgerEntryType::Revenue => "revenue",
            LedgerEntryType::Discount => "discount",
            LedgerEntryType::Refund => "refund",
            LedgerEntryType::LoyaltyEarned => "loyalty_earned",
            LedgerEntryType::LoyaltyRedeemed => "loyalty_redeemed",
        }
    }
}

/// Accounting Projector implementation.
///
/// Writes financial data directly to PostgreSQL.
pub struct AccountingProjector {
    name: String,
    pool: PgPool,
}

impl AccountingProjector {
    /// Create a new accounting projector with the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self {
            name: PROJECTOR_NAME.to_string(),
            pool,
        }
    }

    /// Initialize database schema.
    pub async fn init(&self) -> std::result::Result<(), sqlx::Error> {
        // Create accounting_ledger table
        let create_ledger = Table::create()
            .table(AccountingLedger::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(AccountingLedger::Id)
                    .text()
                    .not_null()
                    .primary_key(),
            )
            .col(ColumnDef::new(AccountingLedger::EntryType).text().not_null())
            .col(ColumnDef::new(AccountingLedger::OrderId).text())
            .col(ColumnDef::new(AccountingLedger::CustomerId).text())
            .col(
                ColumnDef::new(AccountingLedger::AmountCents)
                    .big_integer()
                    .not_null(),
            )
            .col(ColumnDef::new(AccountingLedger::Description).text().not_null())
            .col(ColumnDef::new(AccountingLedger::CorrelationId).text())
            .col(
                ColumnDef::new(AccountingLedger::EventSequence)
                    .integer()
                    .not_null(),
            )
            .col(ColumnDef::new(AccountingLedger::CreatedAt).text().not_null())
            .to_string(PostgresQueryBuilder);

        sqlx::query(&create_ledger).execute(&self.pool).await?;

        // Create index on order_id
        let idx_order = Index::create()
            .if_not_exists()
            .name("idx_accounting_ledger_order")
            .table(AccountingLedger::Table)
            .col(AccountingLedger::OrderId)
            .to_string(PostgresQueryBuilder);

        sqlx::query(&idx_order).execute(&self.pool).await?;

        // Create index on entry_type + created_at
        let idx_type_created = Index::create()
            .if_not_exists()
            .name("idx_accounting_ledger_type_created")
            .table(AccountingLedger::Table)
            .col(AccountingLedger::EntryType)
            .col(AccountingLedger::CreatedAt)
            .to_string(PostgresQueryBuilder);

        sqlx::query(&idx_type_created).execute(&self.pool).await?;

        // Create loyalty_balance table
        let create_loyalty = Table::create()
            .table(LoyaltyBalance::Table)
            .if_not_exists()
            .col(
                ColumnDef::new(LoyaltyBalance::CustomerId)
                    .text()
                    .not_null()
                    .primary_key(),
            )
            .col(
                ColumnDef::new(LoyaltyBalance::CurrentPoints)
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(LoyaltyBalance::LifetimePoints)
                    .big_integer()
                    .not_null(),
            )
            .col(
                ColumnDef::new(LoyaltyBalance::LastSequence)
                    .integer()
                    .not_null(),
            )
            .col(ColumnDef::new(LoyaltyBalance::UpdatedAt).text().not_null())
            .to_string(PostgresQueryBuilder);

        sqlx::query(&create_loyalty).execute(&self.pool).await?;

        Ok(())
    }

    /// Add a ledger entry.
    async fn add_ledger_entry(
        &self,
        entry_type: LedgerEntryType,
        order_id: Option<&str>,
        customer_id: Option<&str>,
        amount_cents: i64,
        description: &str,
        correlation_id: &str,
        event_sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let id = Uuid::new_v4().to_string();
        let created_at = chrono::Utc::now().to_rfc3339();

        let query = Query::insert()
            .into_table(AccountingLedger::Table)
            .columns([
                AccountingLedger::Id,
                AccountingLedger::EntryType,
                AccountingLedger::OrderId,
                AccountingLedger::CustomerId,
                AccountingLedger::AmountCents,
                AccountingLedger::Description,
                AccountingLedger::CorrelationId,
                AccountingLedger::EventSequence,
                AccountingLedger::CreatedAt,
            ])
            .values_panic([
                id.into(),
                entry_type.as_str().into(),
                order_id.map(String::from).into(),
                customer_id.map(String::from).into(),
                amount_cents.into(),
                description.into(),
                correlation_id.into(),
                event_sequence.into(),
                created_at.into(),
            ])
            .to_string(PostgresQueryBuilder);

        sqlx::query(&query).execute(&self.pool).await?;
        Ok(())
    }

    /// Update loyalty balance for a customer.
    async fn update_loyalty_balance(
        &self,
        customer_id: &str,
        points_delta: i64,
        event_sequence: u32,
    ) -> std::result::Result<(), sqlx::Error> {
        let now = chrono::Utc::now().to_rfc3339();

        // Try to get existing balance
        let select_query = Query::select()
            .columns([
                LoyaltyBalance::CurrentPoints,
                LoyaltyBalance::LifetimePoints,
                LoyaltyBalance::LastSequence,
            ])
            .from(LoyaltyBalance::Table)
            .and_where(Expr::col(LoyaltyBalance::CustomerId).eq(customer_id))
            .to_string(PostgresQueryBuilder);

        let existing = sqlx::query(&select_query)
            .fetch_optional(&self.pool)
            .await?;

        match existing {
            Some(row) => {
                let last_seq: i32 = row.get("last_sequence");
                if event_sequence <= last_seq as u32 {
                    // Already processed this event
                    return Ok(());
                }

                let current: i64 = row.get("current_points");
                let lifetime: i64 = row.get("lifetime_points");

                let new_current = current + points_delta;
                let new_lifetime = if points_delta > 0 {
                    lifetime + points_delta
                } else {
                    lifetime
                };

                let update_query = Query::update()
                    .table(LoyaltyBalance::Table)
                    .values([
                        (LoyaltyBalance::CurrentPoints, new_current.into()),
                        (LoyaltyBalance::LifetimePoints, new_lifetime.into()),
                        (LoyaltyBalance::LastSequence, (event_sequence as i32).into()),
                        (LoyaltyBalance::UpdatedAt, now.into()),
                    ])
                    .and_where(Expr::col(LoyaltyBalance::CustomerId).eq(customer_id))
                    .to_string(PostgresQueryBuilder);

                sqlx::query(&update_query).execute(&self.pool).await?;
            }
            None => {
                // Insert new record
                let new_current = points_delta.max(0);
                let new_lifetime = points_delta.max(0);

                let insert_query = Query::insert()
                    .into_table(LoyaltyBalance::Table)
                    .columns([
                        LoyaltyBalance::CustomerId,
                        LoyaltyBalance::CurrentPoints,
                        LoyaltyBalance::LifetimePoints,
                        LoyaltyBalance::LastSequence,
                        LoyaltyBalance::UpdatedAt,
                    ])
                    .values_panic([
                        customer_id.into(),
                        new_current.into(),
                        new_lifetime.into(),
                        (event_sequence as i32).into(),
                        now.into(),
                    ])
                    .on_conflict(
                        OnConflict::column(LoyaltyBalance::CustomerId)
                            .do_nothing()
                            .to_owned(),
                    )
                    .to_string(PostgresQueryBuilder);

                sqlx::query(&insert_query).execute(&self.pool).await?;
            }
        }

        Ok(())
    }

    /// Process a single event and write to database.
    async fn process_event(
        &self,
        event: &prost_types::Any,
        root_id: &str,
        correlation_id: &str,
        sequence: u32,
    ) -> std::result::Result<(), ProjectorError> {
        let type_url = &event.type_url;

        if type_url.ends_with("OrderCreated") {
            if let Ok(created) = OrderCreated::decode(event.value.as_slice()) {
                debug!(order_id = %root_id, subtotal = created.subtotal_cents, "OrderCreated");
                // No ledger entry yet - just tracking the order start
            }
        } else if type_url.ends_with("LoyaltyDiscountApplied") {
            if let Ok(discount) = LoyaltyDiscountApplied::decode(event.value.as_slice()) {
                self.add_ledger_entry(
                    LedgerEntryType::Discount,
                    Some(root_id),
                    None,
                    -(discount.discount_cents as i64),
                    &format!("Loyalty discount: {} points", discount.points_used),
                    correlation_id,
                    sequence,
                )
                .await
                .map_err(|e| ProjectorError::Storage(e.to_string()))?;
            }
        } else if type_url.ends_with("OrderCompleted") {
            if let Ok(completed) = OrderCompleted::decode(event.value.as_slice()) {
                // Record revenue
                self.add_ledger_entry(
                    LedgerEntryType::Revenue,
                    Some(root_id),
                    None,
                    completed.final_total_cents as i64,
                    &format!("Order completed via {}", completed.payment_method),
                    correlation_id,
                    sequence,
                )
                .await
                .map_err(|e| ProjectorError::Storage(e.to_string()))?;

                // Record loyalty points earned
                if completed.loyalty_points_earned > 0 {
                    self.add_ledger_entry(
                        LedgerEntryType::LoyaltyEarned,
                        Some(root_id),
                        None,
                        completed.loyalty_points_earned as i64,
                        "Points earned from order",
                        correlation_id,
                        sequence,
                    )
                    .await
                    .map_err(|e| ProjectorError::Storage(e.to_string()))?;
                }
            }
        } else if type_url.ends_with("OrderCancelled") {
            if let Ok(cancelled) = OrderCancelled::decode(event.value.as_slice()) {
                // Log cancellation - no refund amount in proto
                debug!(
                    order_id = %root_id,
                    reason = %cancelled.reason,
                    loyalty_points_refunded = cancelled.loyalty_points_used,
                    "OrderCancelled"
                );
            }
        } else if type_url.ends_with("LoyaltyPointsAdded") {
            if let Ok(added) = LoyaltyPointsAdded::decode(event.value.as_slice()) {
                self.update_loyalty_balance(root_id, added.points as i64, sequence)
                    .await
                    .map_err(|e| ProjectorError::Storage(e.to_string()))?;
            }
        } else if type_url.ends_with("LoyaltyPointsRedeemed") {
            if let Ok(redeemed) = LoyaltyPointsRedeemed::decode(event.value.as_slice()) {
                self.update_loyalty_balance(root_id, -(redeemed.points as i64), sequence)
                    .await
                    .map_err(|e| ProjectorError::Storage(e.to_string()))?;

                self.add_ledger_entry(
                    LedgerEntryType::LoyaltyRedeemed,
                    None,
                    Some(root_id),
                    redeemed.points as i64,
                    &format!("Points redeemed: {}", redeemed.redemption_type),
                    correlation_id,
                    sequence,
                )
                .await
                .map_err(|e| ProjectorError::Storage(e.to_string()))?;
            }
        }

        Ok(())
    }
}

impl AccountingProjector {
    /// Handle an event book, projecting events to the read model.
    pub async fn handle(&self, book: &EventBook) -> Result<Option<Projection>> {
        let root_id = book
            .cover
            .as_ref()
            .and_then(|c| c.root.as_ref())
            .map(|r| hex::encode(&r.value))
            .unwrap_or_else(|| "unknown".to_string());

        let correlation_id = &book.correlation_id;

        for page in &book.pages {
            let sequence = match &page.sequence {
                Some(angzarr::proto::event_page::Sequence::Num(n)) => *n,
                _ => 0,
            };

            if let Some(event) = &page.event {
                if let Err(e) = self
                    .process_event(event, &root_id, correlation_id, sequence)
                    .await
                {
                    error!(error = %e, event_type = %event.type_url, "Failed to process event");
                }
            }
        }

        // Accounting projector doesn't return sync projections
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_projector_name() {
        // Can't test without database, just verify name
        assert_eq!(PROJECTOR_NAME, "accounting");
    }

    #[test]
    fn test_ledger_entry_type_as_str() {
        assert_eq!(LedgerEntryType::Revenue.as_str(), "revenue");
        assert_eq!(LedgerEntryType::Discount.as_str(), "discount");
        assert_eq!(LedgerEntryType::Refund.as_str(), "refund");
    }
}
