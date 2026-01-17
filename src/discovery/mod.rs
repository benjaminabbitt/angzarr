//! Service discovery for angzarr.
//!
//! Provides mechanisms to discover services:
//! - Unified K8s label-based discovery (recommended, requires service mesh)
//! - Kubernetes API-based discovery (in-cluster)
//! - DNS SRV record-based discovery (K8s headless services)
//! - Static configuration from environment variables (local development)

pub mod coordinator_registry;
pub mod k8s;
pub mod registry;
pub mod srv;
pub mod static_config;
pub mod unified;

// New unified discovery (recommended)
pub use unified::{DiscoveredService, DiscoveryError, ServiceDiscovery};

// Legacy exports (kept for backwards compatibility during transition)
pub use coordinator_registry::{
    load_projector_registry_from_env, load_saga_registry_from_env, CoordinatorEndpoint,
    CoordinatorError, ProjectorRegistry, SagaRegistry,
};
pub use registry::{RegistryError, ServiceEndpoint, ServiceRegistry};
pub use srv::{SrvEndpoint, SrvError, SrvResolver};
