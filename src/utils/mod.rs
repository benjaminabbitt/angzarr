//! Pure utility functions.
//!
//! These are stateless helper functions used across the codebase.

pub mod bootstrap;
#[cfg(feature = "otel")]
pub mod metrics;
pub mod response_builder;
pub mod retry;
pub mod saga_compensation;
pub mod sequence_validator;
pub mod sidecar;
