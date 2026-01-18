//! Database schema definitions for accounting projections.

use sea_query::Iden;

/// Accounting ledger entries for revenue tracking.
#[derive(Iden)]
pub enum AccountingLedger {
    Table,
    #[iden = "id"]
    Id,
    #[iden = "entry_type"]
    EntryType,
    #[iden = "order_id"]
    OrderId,
    #[iden = "customer_id"]
    CustomerId,
    #[iden = "amount_cents"]
    AmountCents,
    #[iden = "description"]
    Description,
    #[iden = "correlation_id"]
    CorrelationId,
    #[iden = "event_sequence"]
    EventSequence,
    #[iden = "created_at"]
    CreatedAt,
}

/// Customer loyalty points balance tracking.
#[derive(Iden)]
pub enum LoyaltyBalance {
    Table,
    #[iden = "customer_id"]
    CustomerId,
    #[iden = "current_points"]
    CurrentPoints,
    #[iden = "lifetime_points"]
    LifetimePoints,
    #[iden = "last_sequence"]
    LastSequence,
    #[iden = "updated_at"]
    UpdatedAt,
}
