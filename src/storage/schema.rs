//! Database schema definitions using sea-query.
//!
//! These define the table and column identifiers for type-safe query building.

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

/// SQL for creating the events table.
pub const CREATE_EVENTS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS events (
    domain TEXT NOT NULL,
    root TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    event_data BLOB NOT NULL,
    synchronous INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (domain, root, sequence)
);

CREATE INDEX IF NOT EXISTS idx_events_domain_root ON events(domain, root);
"#;

/// SQL for creating the snapshots table.
pub const CREATE_SNAPSHOTS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS snapshots (
    domain TEXT NOT NULL,
    root TEXT NOT NULL,
    sequence INTEGER NOT NULL,
    state_data BLOB NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (domain, root)
);
"#;
