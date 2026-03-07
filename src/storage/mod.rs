//! Event and snapshot storage.
//!
//! This module contains:
//! - `EventStore` trait: Event persistence
//! - `SnapshotStore` trait: Snapshot optimization
//! - `PositionStore` trait: Handler checkpoint tracking
//! - Storage configuration types
//! - Implementations: PostgreSQL, SQLite, Redis, Bigtable, DynamoDB, NATS, ImmuDB

// Submodules
pub mod config;
pub mod error;
pub mod factory;

// Trait modules
mod event_store;
mod position_store;
mod snapshot_store;

pub use event_store::{AddOutcome, EventStore, SourceInfo};
pub use position_store::PositionStore;
pub use snapshot_store::SnapshotStore;

// Re-export from submodules
pub use config::{PostgresConfig, RedisConfig, SnapshotsEnableConfig, SqliteConfig, StorageConfig};
pub use error::{errmsg, Result, StorageError};
pub use factory::{init_position_store, init_storage, PositionBackend, StoresBackend};

// Implementation modules
#[cfg(feature = "bigtable")]
pub mod bigtable;
#[cfg(feature = "dynamo")]
pub mod dynamo;
pub mod helpers;
#[cfg(feature = "immudb")]
pub mod immudb;
pub mod mock;
#[cfg(feature = "nats")]
pub mod nats;
#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(feature = "redis")]
pub mod redis;
#[cfg(any(feature = "postgres", feature = "sqlite", feature = "immudb"))]
pub mod schema;
#[cfg(feature = "sqlite")]
pub mod sqlite;
// Unified SQL implementations (shared by postgres and sqlite)
#[cfg(any(feature = "postgres", feature = "sqlite"))]
pub mod sql;

// Backend re-exports
#[cfg(feature = "bigtable")]
pub use bigtable::{
    BigtableConfig, BigtableEventStore, BigtablePositionStore, BigtableSnapshotStore,
};
#[cfg(feature = "dynamo")]
pub use dynamo::{DynamoConfig, DynamoEventStore, DynamoPositionStore, DynamoSnapshotStore};
#[cfg(feature = "immudb")]
pub use immudb::ImmudbEventStore;
pub use mock::{MockEventStore, MockSnapshotStore};
#[cfg(feature = "nats")]
pub use nats::{NatsEventStore, NatsPositionStore, NatsSnapshotStore};
#[cfg(feature = "postgres")]
pub use postgres::{PostgresEventStore, PostgresPositionStore, PostgresSnapshotStore};
#[cfg(feature = "redis")]
pub use redis::RedisSnapshotStore;
#[cfg(feature = "sqlite")]
pub use sqlite::{SqliteEventStore, SqlitePositionStore, SqliteSnapshotStore};
