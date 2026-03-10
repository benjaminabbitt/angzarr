//! Standalone integration tests.
//!
//! Tests the IPC event bus, gRPC over UDS, and embedded runtime integration.
//! Run with: cargo test --test standalone_integration --features sqlite
#![cfg(feature = "sqlite")]

mod cloudevents;
mod common;
mod error;
mod fact_injection;
mod gap_fill;
mod gateway;
mod grpc_uds;
mod ipc;
mod lossy_bus;
mod merge_strategy;
mod process_manager;
mod projector;
mod runtime;
mod saga;
mod snapshot;
mod streaming;
mod sync_mode;
// topology tests disabled until feature is implemented
// mod topology;
mod upcaster;
