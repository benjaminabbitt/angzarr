//! K8s pod watcher for topology descriptor discovery.
//!
//! Watches pods with angzarr component labels and reads their `angzarr.io/descriptor`
//! annotations to build the topology graph. This replaces event bus-based descriptor
//! discovery for K8s-native topology visualization.

use std::sync::Arc;

use futures::TryStreamExt;
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::Api,
    runtime::watcher::{self, Event},
    Client,
};
use tracing::{debug, error, info, warn};

use crate::discovery::k8s::DESCRIPTOR_ANNOTATION;
use crate::proto::ComponentDescriptor;

use super::store::TopologyStore;
use super::TopologyProjector;

/// Label for component type.
const COMPONENT_LABEL: &str = "app.kubernetes.io/component";

/// Component type values we watch for.
const WATCHED_COMPONENTS: &[&str] = &["aggregate", "saga", "projector", "process-manager"];

/// K8s pod watcher for topology discovery.
///
/// Watches pods with angzarr component labels and registers their descriptors
/// with the topology projector.
pub struct TopologyK8sWatcher {
    client: Client,
    namespace: String,
    projector: Arc<TopologyProjector>,
    store: Arc<dyn TopologyStore>,
}

impl TopologyK8sWatcher {
    /// Create a new K8s watcher.
    ///
    /// # Arguments
    /// * `client` - K8s client
    /// * `namespace` - Namespace to watch
    /// * `projector` - Topology projector to register descriptors with
    /// * `store` - Topology store for node deletion
    pub fn new(
        client: Client,
        namespace: String,
        projector: Arc<TopologyProjector>,
        store: Arc<dyn TopologyStore>,
    ) -> Self {
        Self {
            client,
            namespace,
            projector,
            store,
        }
    }

    /// Create from environment variables.
    ///
    /// Reads namespace from POD_NAMESPACE or NAMESPACE env vars.
    pub async fn from_env(
        projector: Arc<TopologyProjector>,
        store: Arc<dyn TopologyStore>,
    ) -> Result<Self, kube::Error> {
        let client = Client::try_default().await?;
        let namespace = std::env::var(crate::config::POD_NAMESPACE_ENV_VAR)
            .or_else(|_| std::env::var(crate::config::NAMESPACE_ENV_VAR))
            .unwrap_or_else(|_| "default".to_string());

        Ok(Self::new(client, namespace, projector, store))
    }

    /// Run the watcher loop.
    ///
    /// Watches pods with component labels and processes descriptor annotations.
    /// This method runs indefinitely until an error occurs.
    pub async fn run(&self) -> Result<(), watcher::Error> {
        let pods: Api<Pod> = Api::namespaced(self.client.clone(), &self.namespace);

        // Watch all pods and filter by component label in handle_event
        // (K8s label selectors don't support OR for multiple component types)
        let watcher = watcher::watcher(pods, watcher::Config::default());

        info!(
            namespace = %self.namespace,
            "Starting topology K8s pod watcher"
        );

        watcher
            .try_for_each(|event| async {
                self.handle_event(event).await;
                Ok(())
            })
            .await
    }

    /// Handle a pod watch event.
    async fn handle_event(&self, event: Event<Pod>) {
        match event {
            Event::Apply(pod) | Event::InitApply(pod) => {
                self.handle_pod_apply(&pod).await;
            }
            Event::Delete(pod) => {
                self.handle_pod_delete(&pod).await;
            }
            Event::Init => {
                debug!("Pod watcher initialized");
            }
            Event::InitDone => {
                debug!("Pod watcher init done");
            }
        }
    }

    /// Handle pod creation or update.
    async fn handle_pod_apply(&self, pod: &Pod) {
        let pod_name = match pod.metadata.name.as_ref() {
            Some(n) => n,
            None => return,
        };

        // Check if this is a component we care about
        let labels = match pod.metadata.labels.as_ref() {
            Some(l) => l,
            None => return,
        };

        let component_type = match labels.get(COMPONENT_LABEL) {
            Some(c) if WATCHED_COMPONENTS.contains(&c.as_str()) => c,
            _ => return,
        };

        // Get descriptor annotation
        let annotations = match pod.metadata.annotations.as_ref() {
            Some(a) => a,
            None => {
                debug!(
                    pod = %pod_name,
                    component_type = %component_type,
                    "Pod has no annotations, waiting for descriptor"
                );
                return;
            }
        };

        let descriptor_json = match annotations.get(DESCRIPTOR_ANNOTATION) {
            Some(d) => d,
            None => {
                debug!(
                    pod = %pod_name,
                    component_type = %component_type,
                    "Pod has no descriptor annotation yet"
                );
                return;
            }
        };

        // Parse descriptor
        let descriptor: ComponentDescriptor = match serde_json::from_str(descriptor_json) {
            Ok(d) => d,
            Err(e) => {
                warn!(
                    pod = %pod_name,
                    error = %e,
                    "Failed to parse descriptor annotation"
                );
                return;
            }
        };

        info!(
            pod = %pod_name,
            descriptor_name = %descriptor.name,
            component_type = %descriptor.component_type,
            inputs = descriptor.inputs.len(),
            outputs = descriptor.outputs.len(),
            "Discovered component from K8s annotation"
        );

        // Register with topology projector
        if let Err(e) = self.projector.register_components(&[descriptor]).await {
            error!(
                pod = %pod_name,
                error = %e,
                "Failed to register component in topology"
            );
        }
    }

    /// Handle pod deletion.
    async fn handle_pod_delete(&self, pod: &Pod) {
        let pod_name = match pod.metadata.name.as_ref() {
            Some(n) => n,
            None => return,
        };

        // Check if this was a component we track
        let labels = match pod.metadata.labels.as_ref() {
            Some(l) => l,
            None => return,
        };

        let component_type = match labels.get(COMPONENT_LABEL) {
            Some(c) if WATCHED_COMPONENTS.contains(&c.as_str()) => c,
            _ => return,
        };

        // Get the descriptor name from annotation (if still available)
        let node_id = pod
            .metadata
            .annotations
            .as_ref()
            .and_then(|a| a.get(DESCRIPTOR_ANNOTATION))
            .and_then(|json| serde_json::from_str::<ComponentDescriptor>(json).ok())
            .map(|d| d.name);

        if let Some(node_id) = node_id {
            info!(
                pod = %pod_name,
                node_id = %node_id,
                component_type = %component_type,
                "Removing component from topology (pod deleted)"
            );

            if let Err(e) = self.store.delete_node(&node_id).await {
                error!(
                    node_id = %node_id,
                    error = %e,
                    "Failed to delete node from topology"
                );
            }
        } else {
            debug!(
                pod = %pod_name,
                component_type = %component_type,
                "Pod deleted but no descriptor annotation found"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;
    use std::collections::BTreeMap;

    #[test]
    fn test_watched_components() {
        assert!(WATCHED_COMPONENTS.contains(&"aggregate"));
        assert!(WATCHED_COMPONENTS.contains(&"saga"));
        assert!(WATCHED_COMPONENTS.contains(&"projector"));
        assert!(WATCHED_COMPONENTS.contains(&"process-manager"));
    }

    #[test]
    fn test_descriptor_json_parsing() {
        let json = r#"{"name":"sag-order-fulfillment","component_type":"saga","inputs":[{"domain":"order","types":["OrderCompleted"]}],"outputs":[{"domain":"fulfillment","types":["CreateShipment"]}]}"#;
        let descriptor: ComponentDescriptor = serde_json::from_str(json).unwrap();

        assert_eq!(descriptor.name, "sag-order-fulfillment");
        assert_eq!(descriptor.component_type, "saga");
        assert_eq!(descriptor.inputs.len(), 1);
        assert_eq!(descriptor.inputs[0].domain, "order");
        assert_eq!(descriptor.inputs[0].types, vec!["OrderCompleted"]);
        assert_eq!(descriptor.outputs.len(), 1);
        assert_eq!(descriptor.outputs[0].domain, "fulfillment");
        assert_eq!(descriptor.outputs[0].types, vec!["CreateShipment"]);
    }

    #[test]
    fn test_aggregate_descriptor_parsing() {
        let json = r#"{"name":"order","component_type":"aggregate","inputs":[],"outputs":[]}"#;
        let descriptor: ComponentDescriptor = serde_json::from_str(json).unwrap();

        assert_eq!(descriptor.name, "order");
        assert_eq!(descriptor.component_type, "aggregate");
        assert!(descriptor.inputs.is_empty());
        assert!(descriptor.outputs.is_empty());
    }

    fn make_test_pod(name: &str, component: &str, descriptor_json: Option<&str>) -> Pod {
        let mut labels = BTreeMap::new();
        labels.insert(COMPONENT_LABEL.to_string(), component.to_string());

        let mut annotations = BTreeMap::new();
        if let Some(json) = descriptor_json {
            annotations.insert(DESCRIPTOR_ANNOTATION.to_string(), json.to_string());
        }

        Pod {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some("test-ns".to_string()),
                labels: Some(labels),
                annotations: if annotations.is_empty() {
                    None
                } else {
                    Some(annotations)
                },
                ..Default::default()
            },
            spec: None,
            status: None,
        }
    }

    #[test]
    fn test_pod_with_descriptor_annotation() {
        let descriptor_json = r#"{"name":"order","component_type":"aggregate","inputs":[],"outputs":[]}"#;
        let pod = make_test_pod("order-pod-abc123", "aggregate", Some(descriptor_json));

        // Verify pod structure
        assert_eq!(pod.metadata.name, Some("order-pod-abc123".to_string()));

        let labels = pod.metadata.labels.as_ref().unwrap();
        assert_eq!(labels.get(COMPONENT_LABEL), Some(&"aggregate".to_string()));

        let annotations = pod.metadata.annotations.as_ref().unwrap();
        let desc_json = annotations.get(DESCRIPTOR_ANNOTATION).unwrap();
        let descriptor: ComponentDescriptor = serde_json::from_str(desc_json).unwrap();
        assert_eq!(descriptor.name, "order");
    }

    #[test]
    fn test_pod_without_annotation() {
        let pod = make_test_pod("order-pod-abc123", "aggregate", None);

        assert!(pod.metadata.annotations.is_none());
    }

    #[test]
    fn test_component_label_filtering() {
        // Verify only watched components pass the filter
        for component in WATCHED_COMPONENTS {
            let pod = make_test_pod("test-pod", component, None);
            let labels = pod.metadata.labels.as_ref().unwrap();
            let component_type = labels.get(COMPONENT_LABEL);
            assert!(
                component_type.is_some_and(|c| WATCHED_COMPONENTS.contains(&c.as_str())),
                "Component {} should be in watched list",
                component
            );
        }

        // Non-watched component should not match
        let pod = make_test_pod("test-pod", "infrastructure", None);
        let labels = pod.metadata.labels.as_ref().unwrap();
        let component_type = labels.get(COMPONENT_LABEL);
        assert!(
            component_type.is_none_or(|c| !WATCHED_COMPONENTS.contains(&c.as_str())),
            "infrastructure should not be in watched list"
        );
    }

    #[test]
    fn test_extract_descriptor_from_pod() {
        let descriptor_json = r#"{"name":"sag-order-fulfillment","component_type":"saga","inputs":[{"domain":"order","types":["OrderCompleted"]}],"outputs":[{"domain":"fulfillment","types":["CreateShipment"]}]}"#;
        let pod = make_test_pod("saga-pod-xyz", "saga", Some(descriptor_json));

        // Simulate what handle_pod_apply does
        let annotations = pod.metadata.annotations.as_ref().unwrap();
        let json = annotations.get(DESCRIPTOR_ANNOTATION).unwrap();
        let descriptor: ComponentDescriptor = serde_json::from_str(json).unwrap();

        assert_eq!(descriptor.name, "sag-order-fulfillment");
        assert_eq!(descriptor.component_type, "saga");
        assert_eq!(descriptor.inputs[0].domain, "order");
        assert_eq!(descriptor.outputs[0].domain, "fulfillment");
    }
}
