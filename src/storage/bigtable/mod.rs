//! Google Cloud Bigtable storage implementations.
//!
//! Uses Bigtable for event sourcing storage with the following table structure:
//!
//! ## Events Table
//! - Row key: `{domain}#{edition}#{root}#{sequence:010}`
//! - Column family: `event`
//!   - `data`: serialized EventPage (Binary)
//!   - `created_at`: ISO 8601 timestamp (String)
//!   - `correlation_id`: for cross-domain queries (String)
//!
//! ## Snapshots Table
//! - Row key: `{domain}#{edition}#{root}#{sequence:010}`
//! - Column family: `snapshot`
//!   - `data`: serialized Snapshot (Binary)
//!   - `retention`: retention type (String)
//!
//! ## Positions Table
//! - Row key: `{handler}#{domain}#{edition}#{root_hex}`
//! - Column family: `position`
//!   - `sequence`: last processed sequence number (String)

mod event_store;
mod position_store;
mod snapshot_store;

#[cfg(test)]
mod tests;

pub use event_store::BigtableEventStore;
pub use position_store::BigtablePositionStore;
pub use snapshot_store::BigtableSnapshotStore;

use serde::Deserialize;

/// Bigtable configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BigtableConfig {
    /// GCP project ID.
    pub project_id: String,
    /// Bigtable instance ID.
    pub instance_id: String,
    /// Events table name.
    pub events_table: String,
    /// Snapshots table name.
    pub snapshots_table: String,
    /// Positions table name.
    pub positions_table: String,
    /// Emulator host for local development (optional).
    pub emulator_host: Option<String>,
}

impl Default for BigtableConfig {
    fn default() -> Self {
        Self {
            project_id: "".to_string(),
            instance_id: "angzarr".to_string(),
            events_table: "events".to_string(),
            snapshots_table: "snapshots".to_string(),
            positions_table: "positions".to_string(),
            emulator_host: None,
        }
    }
}
