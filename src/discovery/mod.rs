//! Service discovery for angzarr gateway.
//!
//! Provides mechanisms to discover command handler services:
//! - Kubernetes API-based discovery (in-cluster)
//! - Static configuration from environment variables (local development)

pub mod k8s;
pub mod registry;
pub mod static_config;

pub use registry::{RegistryError, ServiceEndpoint, ServiceRegistry};
