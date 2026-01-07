//! Storage implementations.

pub mod schema;
pub mod sqlite;

#[cfg(feature = "redis")]
pub mod redis;

pub use sqlite::{SqliteEventStore, SqliteSnapshotStore};

#[cfg(feature = "redis")]
pub use redis::RedisEventStore;
