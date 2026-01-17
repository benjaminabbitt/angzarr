//! Database schema definitions using sea-query.
//!
//! These define the table and column identifiers for type-safe query building.
//! Schema creation is handled via sea-query's Table::create() in each backend.

use sea_query::Iden;

/// Events table schema.
#[derive(Iden)]
pub enum Events {
    Table,
    #[iden = "domain"]
    Domain,
    #[iden = "root"]
    Root,
    #[iden = "sequence"]
    Sequence,
    #[iden = "created_at"]
    CreatedAt,
    #[iden = "event_data"]
    EventData,
    #[iden = "synchronous"]
    Synchronous,
}

/// Snapshots table schema.
#[derive(Iden)]
pub enum Snapshots {
    Table,
    #[iden = "domain"]
    Domain,
    #[iden = "root"]
    Root,
    #[iden = "sequence"]
    Sequence,
    #[iden = "state_data"]
    StateData,
    #[iden = "created_at"]
    CreatedAt,
}
