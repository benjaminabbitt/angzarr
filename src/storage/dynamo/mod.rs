//! DynamoDB storage implementations.
//!
//! Uses DynamoDB for event sourcing storage with the following table structure:
//!
//! ## Events Table
//! - Partition Key: `pk` (String) - Format: `{domain}#{edition}#{root}`
//! - Sort Key: `seq` (Number) - Sequence number
//! - Attributes: `event` (Binary), `created_at` (String), `correlation_id` (String)
//! - GSI: `correlation-index` on `correlation_id` for cross-domain queries
//!
//! ## Snapshots Table
//! - Partition Key: `pk` (String) - Format: `{domain}#{edition}#{root}`
//! - Sort Key: `seq` (Number) - Snapshot sequence number
//! - Attributes: `snapshot` (Binary)
//!
//! ## Positions Table
//! - Partition Key: `pk` (String) - Format: `{handler}#{domain}#{edition}#{root_hex}`
//! - Attributes: `sequence` (Number)

mod event_store;
mod position_store;
mod snapshot_store;

pub use event_store::DynamoEventStore;
pub use position_store::DynamoPositionStore;
pub use snapshot_store::DynamoSnapshotStore;

use serde::Deserialize;

/// DynamoDB configuration.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DynamoConfig {
    /// AWS region (defaults to us-east-1).
    pub region: String,
    /// Events table name.
    pub events_table: String,
    /// Snapshots table name.
    pub snapshots_table: String,
    /// Positions table name.
    pub positions_table: String,
    /// Custom endpoint URL (for local DynamoDB or LocalStack).
    pub endpoint_url: Option<String>,
}

impl Default for DynamoConfig {
    fn default() -> Self {
        Self {
            region: "us-east-1".to_string(),
            events_table: "angzarr-events".to_string(),
            snapshots_table: "angzarr-snapshots".to_string(),
            positions_table: "angzarr-positions".to_string(),
            endpoint_url: None,
        }
    }
}
