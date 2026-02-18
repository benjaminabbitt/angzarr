//! NATS JetStream storage backend.
//!
//! Provides `EventStore`, `PositionStore`, and `SnapshotStore` implementations
//! using NATS JetStream streams and KV buckets.
//!
//! ## Architecture
//!
//! - **EventStore**: JetStream streams with per-aggregate subjects
//!   - Stream per domain: `ANGZARR_{DOMAIN}`
//!   - Subject: `{prefix}.events.{domain}.{root}.{edition}`
//!   - Sequence numbers stored in message headers
//!
//! - **PositionStore**: JetStream KV bucket
//!   - Bucket: `{prefix}-positions`
//!   - Key: `{handler}.{domain}.{root}.{edition}`
//!
//! - **SnapshotStore**: JetStream KV bucket with history
//!   - Bucket: `{prefix}-snapshots`
//!   - Key: `{domain}.{root}.{edition}`
//!   - History enabled for `get_at_seq()` support

mod event_store;
mod position_store;
mod snapshot_store;

pub use event_store::NatsEventStore;
pub use position_store::NatsPositionStore;
pub use snapshot_store::NatsSnapshotStore;

/// Default subject prefix for NATS streams.
pub const DEFAULT_PREFIX: &str = "angzarr";

/// Default edition name (main timeline).
pub const DEFAULT_EDITION: &str = "angzarr";
