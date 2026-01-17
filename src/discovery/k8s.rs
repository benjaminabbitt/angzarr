//! Kubernetes-based service discovery.
//!
//! Discovers command handler services by watching Kubernetes Service resources
//! with the appropriate labels and annotations.

use std::sync::Arc;

use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Service;
use kube::{
    api::{Api, ListParams},
    runtime::watcher::{self, Event},
    Client,
};
use tracing::{debug, error, info};

use super::registry::{ServiceEndpoint, ServiceRegistry};

/// Label used to identify command handler services.
const COMPONENT_LABEL: &str = "app.kubernetes.io/component";
const COMPONENT_VALUE: &str = "business";

/// Annotation containing the domain name for routing.
const DOMAIN_ANNOTATION: &str = "angzarr.io/domain";

/// Default gRPC port if not specified in service.
const DEFAULT_GRPC_PORT: u16 = 50051;

/// Kubernetes-based service discovery.
///
/// Watches K8s Service resources and updates the service registry
/// when services are added, modified, or removed.
pub struct K8sServiceDiscovery {
    client: Client,
    registry: Arc<ServiceRegistry>,
    namespace: String,
}

impl K8sServiceDiscovery {
    /// Create a new K8s service discovery instance.
    ///
    /// Uses the in-cluster configuration automatically.
    pub async fn new(registry: Arc<ServiceRegistry>) -> Result<Self, kube::Error> {
        let client = Client::try_default().await?;

        // Get namespace from env or use default
        let namespace = std::env::var("NAMESPACE")
            .or_else(|_| std::env::var("POD_NAMESPACE"))
            .unwrap_or_else(|_| "default".to_string());

        info!(namespace = %namespace, "K8s service discovery initialized");

        Ok(Self {
            client,
            registry,
            namespace,
        })
    }

    /// Perform initial sync of all services.
    ///
    /// Lists all services matching the label selector and populates
    /// the registry. Should be called before starting the watcher.
    pub async fn initial_sync(&self) -> Result<(), kube::Error> {
        let services: Api<Service> = Api::namespaced(self.client.clone(), &self.namespace);
        let lp = ListParams::default().labels(&format!("{}={}", COMPONENT_LABEL, COMPONENT_VALUE));

        info!("Performing initial service sync");

        let service_list = services.list(&lp).await?;
        let mut count = 0;

        for svc in service_list {
            if let Some(endpoint) = self.service_to_endpoint(&svc) {
                self.registry.update_endpoint(endpoint).await;
                count += 1;
            }
        }

        info!(count = count, "Initial sync complete");
        Ok(())
    }

    /// Start watching for service changes in the background.
    ///
    /// Spawns a tokio task that watches for service events and updates
    /// the registry accordingly. Returns immediately.
    pub fn start_watching(&self) {
        let client = self.client.clone();
        let registry = self.registry.clone();
        let namespace = self.namespace.clone();

        tokio::spawn(async move {
            let services: Api<Service> = Api::namespaced(client, &namespace);

            let watcher = watcher::watcher(
                services,
                watcher::Config::default()
                    .labels(&format!("{}={}", COMPONENT_LABEL, COMPONENT_VALUE)),
            );

            info!("Starting K8s service watcher");

            if let Err(e) = watcher
                .try_for_each(|event| {
                    let registry = registry.clone();
                    async move {
                        Self::handle_event(&registry, event).await;
                        Ok(())
                    }
                })
                .await
            {
                error!(error = %e, "Service watcher error");
            }
        });
    }

    /// Handle a watcher event.
    async fn handle_event(registry: &ServiceRegistry, event: Event<Service>) {
        match event {
            Event::Apply(svc) | Event::InitApply(svc) => {
                if let Some(endpoint) = Self::extract_endpoint(&svc) {
                    debug!(domain = %endpoint.domain, "Service applied/updated");
                    registry.update_endpoint(endpoint).await;
                }
            }
            Event::Delete(svc) => {
                if let Some(name) = svc.metadata.name {
                    // Try to get domain from annotations, fall back to service name
                    let domain = svc
                        .metadata
                        .annotations
                        .as_ref()
                        .and_then(|a| a.get(DOMAIN_ANNOTATION))
                        .cloned()
                        .unwrap_or(name.clone());

                    debug!(domain = %domain, service = %name, "Service deleted");
                    registry.remove_endpoint(&domain).await;
                }
            }
            Event::Init => {
                debug!("Watcher initialized");
            }
            Event::InitDone => {
                debug!("Watcher init done");
            }
        }
    }

    /// Extract endpoint from a Service resource.
    fn extract_endpoint(svc: &Service) -> Option<ServiceEndpoint> {
        let name = svc.metadata.name.as_ref()?;
        let annotations = svc.metadata.annotations.as_ref();

        // Get domain from annotation, default to service name
        let domain = annotations
            .and_then(|a| a.get(DOMAIN_ANNOTATION))
            .cloned()
            .unwrap_or_else(|| name.clone());

        // Get cluster IP or service name for address
        let address = svc
            .spec
            .as_ref()
            .and_then(|s| s.cluster_ip.clone())
            .filter(|ip| ip != "None")
            .unwrap_or_else(|| name.clone());

        // Find command port - look for port named "command" (angzarr sidecar)
        // Business services expose: command (50051), query (50052), grpc (app port)
        // Gateway routes to the sidecar's command port, not directly to the app
        let port = svc
            .spec
            .as_ref()
            .and_then(|s| s.ports.as_ref())
            .and_then(|ports| {
                // Prefer port named "command" (angzarr sidecar command handler)
                ports
                    .iter()
                    .find(|p| p.name.as_deref() == Some("command"))
                    .or_else(|| ports.first())
            })
            .and_then(|p| u16::try_from(p.port).ok())
            .unwrap_or(DEFAULT_GRPC_PORT);

        info!(
            domain = %domain,
            address = %address,
            port = port,
            service = %name,
            "Discovered service"
        );

        Some(ServiceEndpoint {
            domain,
            address,
            port,
        })
    }

    /// Convert a Service to an endpoint (for initial sync).
    fn service_to_endpoint(&self, svc: &Service) -> Option<ServiceEndpoint> {
        Self::extract_endpoint(svc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::api::core::v1::{ServicePort, ServiceSpec};
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    fn make_test_service(name: &str, domain: Option<&str>, port: i32) -> Service {
        let mut annotations = BTreeMap::new();
        if let Some(d) = domain {
            annotations.insert(DOMAIN_ANNOTATION.to_string(), d.to_string());
        }

        Service {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                annotations: if annotations.is_empty() {
                    None
                } else {
                    Some(annotations)
                },
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                cluster_ip: Some("10.0.0.1".to_string()),
                ports: Some(vec![ServicePort {
                    name: Some("command".to_string()),
                    port,
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            status: None,
        }
    }

    #[test]
    fn test_extract_endpoint_with_annotation() {
        let svc = make_test_service("angzarr-customer", Some("customer"), 50051);
        let endpoint = K8sServiceDiscovery::extract_endpoint(&svc).unwrap();

        assert_eq!(endpoint.domain, "customer");
        assert_eq!(endpoint.address, "10.0.0.1");
        assert_eq!(endpoint.port, 50051);
    }

    #[test]
    fn test_extract_endpoint_without_annotation() {
        let svc = make_test_service("angzarr-orders", None, 50052);
        let endpoint = K8sServiceDiscovery::extract_endpoint(&svc).unwrap();

        // Should fall back to service name
        assert_eq!(endpoint.domain, "angzarr-orders");
        assert_eq!(endpoint.address, "10.0.0.1");
        assert_eq!(endpoint.port, 50052);
    }

    #[test]
    fn test_extract_endpoint_uses_service_name_when_no_cluster_ip() {
        let svc = Service {
            metadata: ObjectMeta {
                name: Some("angzarr-customer".to_string()),
                annotations: Some({
                    let mut a = BTreeMap::new();
                    a.insert(DOMAIN_ANNOTATION.to_string(), "customer".to_string());
                    a
                }),
                ..Default::default()
            },
            spec: Some(ServiceSpec {
                cluster_ip: Some("None".to_string()), // Headless service
                ports: Some(vec![ServicePort {
                    port: 50051,
                    ..Default::default()
                }]),
                ..Default::default()
            }),
            status: None,
        };

        let endpoint = K8sServiceDiscovery::extract_endpoint(&svc).unwrap();
        assert_eq!(endpoint.address, "angzarr-customer");
    }
}
