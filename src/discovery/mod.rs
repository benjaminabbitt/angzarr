//! Service discovery for angzarr.
//!
//! Uses K8s label-based discovery with service mesh for L7 gRPC load balancing.
//! Services are discovered by watching K8s Service resources with labels:
//! - `app.kubernetes.io/component`: aggregate, projector, or saga
//! - `angzarr.io/domain`: target domain
//! - `angzarr.io/source-domain`: source domain (sagas only)

pub mod k8s;

pub use k8s::{DiscoveredService, DiscoveryError, K8sServiceDiscovery};

use tonic::transport::Channel;

use crate::proto::aggregate_coordinator_service_client::AggregateCoordinatorServiceClient;
use crate::proto::event_query_service_client::EventQueryServiceClient;
use crate::proto::projector_coordinator_service_client::ProjectorCoordinatorServiceClient;

/// Trait for service discovery.
///
/// Abstracts over concrete discovery mechanisms (K8s, static, etc.)
/// to enable testing and alternative implementations.
#[async_trait::async_trait]
pub trait ServiceDiscovery: Send + Sync {
    /// Register an aggregate service manually.
    async fn register_aggregate(&self, domain: &str, address: &str, port: u16);

    /// Register a projector service manually.
    async fn register_projector(&self, name: &str, domain: &str, address: &str, port: u16);

    /// Get aggregate coordinator client by domain.
    async fn get_aggregate(
        &self,
        domain: &str,
    ) -> Result<AggregateCoordinatorServiceClient<Channel>, DiscoveryError>;

    /// Get event query client by domain.
    async fn get_event_query(
        &self,
        domain: &str,
    ) -> Result<EventQueryServiceClient<Channel>, DiscoveryError>;

    /// Get all projector clients.
    async fn get_all_projectors(
        &self,
    ) -> Result<Vec<ProjectorCoordinatorServiceClient<Channel>>, DiscoveryError>;

    /// Get projector client by name.
    async fn get_projector_by_name(
        &self,
        name: &str,
    ) -> Result<ProjectorCoordinatorServiceClient<Channel>, DiscoveryError>;

    /// Get all aggregate domains.
    async fn aggregate_domains(&self) -> Vec<String>;

    /// Check if any aggregates are available.
    async fn has_aggregates(&self) -> bool;

    /// Check if any projectors are available.
    async fn has_projectors(&self) -> bool;

    /// Perform initial sync of all services.
    async fn initial_sync(&self) -> Result<(), DiscoveryError>;

    /// Start watching for service changes in the background.
    fn start_watching(&self);
}
