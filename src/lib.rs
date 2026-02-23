//! Angzarr - CQRS/ES Framework
//!
//! A Rust implementation of the angzarr framework for building
//! event-sourced applications with CQRS architecture.

#[cfg(feature = "advice")]
pub mod advice;
pub mod bus;
pub mod clients;
pub mod config;
pub mod descriptor;
pub mod discovery;
pub mod dlq;
pub mod edition;
pub mod grpc;
pub mod handlers;
pub mod orchestration;
pub mod payload_store;
pub mod process;
pub mod proto_ext;
pub mod proto_reflect;
pub mod registration;
pub mod repository;
pub mod services;
#[cfg(feature = "sqlite")]
pub mod standalone;
pub mod storage;
pub mod transport;
pub mod utils;
pub mod validation;

/// Test utilities for bus and storage integration tests.
///
/// Provides reusable test handlers and fixture builders.
#[cfg(any(test, feature = "test-utils"))]
pub mod test_utils;

// Re-export proto types from angzarr-client (includes both client and server)
pub use angzarr_client::proto;

// Re-export extension traits (our proto_ext module re-exports from angzarr-client + adds framework-specific)
pub use angzarr_client::{
    CommandBookExt, CommandPageExt, CoverExt, EditionExt, EventBookExt, EventPageExt, ProtoUuidExt,
    UuidExt,
};

// Re-export client traits from angzarr-client
pub mod client_traits {
    pub use angzarr_client::error::{ClientError, Result};
    pub use angzarr_client::traits::{GatewayClient, QueryClient, SpeculativeClient};
}
