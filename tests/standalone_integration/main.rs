//! Standalone integration tests.
//!
//! Tests the IPC event bus, gRPC over UDS, and embedded runtime integration.
//! Run with: cargo test --test standalone_integration --features sqlite
#![cfg(feature = "sqlite")]

mod common;
mod error;
mod event_book_repair;
mod gateway;
mod grpc_uds;
mod ipc;
mod lossy_bus;
mod process_manager;
mod projector;
mod runtime;
mod saga;
mod snapshot;
mod streaming;
#[cfg(feature = "topology")]
mod topology;
