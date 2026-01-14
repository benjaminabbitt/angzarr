//! Event handlers for sidecar binaries.

#[cfg(feature = "mode-projector")]
pub mod projector;

#[cfg(feature = "mode-saga")]
pub mod saga;

#[cfg(feature = "mode-stream")]
pub mod stream;

#[cfg(feature = "mode-gateway")]
pub mod gateway;
