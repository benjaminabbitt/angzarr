//! Angzarr - CQRS/ES Framework
//!
//! A Rust implementation of the angzarr framework for building
//! event-sourced applications with CQRS architecture.

pub mod bus;
pub mod clients;
pub mod config;
pub mod discovery;
pub mod handlers;
pub mod interfaces;
pub mod projectors;
pub mod repository;
pub mod sagas;
pub mod services;
pub mod storage;

#[cfg(test)]
pub mod test_utils;

// Re-export generated proto types
pub mod proto {
    tonic::include_proto!("angzarr");
}

// Re-export async_trait for implementors
pub use async_trait;

// Re-export common types for library usage
pub use config::Config;
pub use interfaces::{BusinessLogicClient, EventBus, EventStore, Projector, Saga, SnapshotStore};
pub use proto::{CommandBook, ContextualCommand, Cover, EventBook, EventPage, Snapshot};
