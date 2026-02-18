//! Extension traits for proto types.
//!
//! Most extension traits are provided by angzarr-client and re-exported here.
//! This module adds framework-specific functionality like otel-enhanced gRPC utilities.

pub mod grpc;

// Re-export everything from angzarr-client
pub use angzarr_client::proto_ext::*;

// Re-export framework-specific utilities
pub use grpc::correlated_request;
