//! Database schema definitions using sea-query.
//!
//! These define the table and column identifiers for type-safe query building.
//! Schema creation is handled via sqlx migrations (see `migrations/`).

use sea_query::Iden;

/// Events table schema.
#[derive(Iden)]
pub enum Events {
    Table,
    #[iden = "domain"]
    Domain,
    #[iden = "edition"]
    Edition,
    #[iden = "root"]
    Root,
    #[iden = "sequence"]
    Sequence,
    #[iden = "created_at"]
    CreatedAt,
    #[iden = "event_data"]
    EventData,
    #[iden = "correlation_id"]
    CorrelationId,
    #[iden = "external_id"]
    ExternalId,
    // Source tracking for saga-produced events (angzarr_deferred)
    #[iden = "source_edition"]
    SourceEdition,
    #[iden = "source_domain"]
    SourceDomain,
    #[iden = "source_root"]
    SourceRoot,
    #[iden = "source_seq"]
    SourceSeq,
    // Cascade tracking for 2PC (Phase 5)
    #[iden = "committed"]
    Committed,
    #[iden = "cascade_id"]
    CascadeId,
}

/// Snapshots table schema.
#[derive(Iden)]
pub enum Snapshots {
    Table,
    #[iden = "domain"]
    Domain,
    #[iden = "edition"]
    Edition,
    #[iden = "root"]
    Root,
    #[iden = "sequence"]
    Sequence,
    #[iden = "state_data"]
    StateData,
    #[iden = "created_at"]
    CreatedAt,
}

/// Positions table schema.
///
/// Tracks last-processed event sequence per handler/domain/edition/root.
/// Used by projectors and sagas to resume from their last checkpoint.
#[derive(Iden)]
pub enum Positions {
    Table,
    #[iden = "handler"]
    Handler,
    #[iden = "domain"]
    Domain,
    #[iden = "edition"]
    Edition,
    #[iden = "root"]
    Root,
    #[iden = "sequence"]
    Sequence,
    #[iden = "updated_at"]
    UpdatedAt,
}

/// Editions table schema.
///
/// Stores metadata for diverged timelines. Each edition forks the main
/// timeline at a divergence point (sequence number or timestamp) and
/// continues independently.
#[derive(Iden)]
pub enum Editions {
    Table,
    #[iden = "name"]
    Name,
    #[iden = "divergence_point_type"]
    DivergencePointType,
    #[iden = "divergence_point_value"]
    DivergencePointValue,
    #[iden = "description"]
    Description,
    #[iden = "created_at"]
    CreatedAt,
}
