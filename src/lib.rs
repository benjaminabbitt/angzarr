//! Evented - CQRS/ES Framework
//!
//! A Rust implementation of the evented framework for building
//! event-sourced applications with CQRS architecture.

pub mod bus;
pub mod clients;
pub mod config;
pub mod facade;
pub mod interfaces;
pub mod projectors;
pub mod repository;
pub mod sagas;
pub mod services;
pub mod storage;

// Re-export generated proto types
pub mod proto {
    tonic::include_proto!("evented");
}

// Re-export common types for library usage
pub use config::Config;
pub use facade::{Evented, EventedBuilder, EventedConfig, EventedError};
pub use interfaces::{BusinessLogicClient, EventBus, EventStore, Projector, Saga, SnapshotStore};
pub use proto::{CommandBook, ContextualCommand, Cover, EventBook, EventPage, Snapshot};
