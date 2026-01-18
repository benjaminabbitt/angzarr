//! Service discovery for angzarr.
//!
//! Uses K8s label-based discovery with service mesh for L7 gRPC load balancing.
//! Services are discovered by watching K8s Service resources with labels:
//! - `app.kubernetes.io/component`: aggregate, projector, or saga
//! - `angzarr.io/domain`: target domain
//! - `angzarr.io/source-domain`: source domain (sagas only)

pub mod k8s;

pub use k8s::{DiscoveredService, DiscoveryError, ServiceDiscovery};
