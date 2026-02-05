//! K8s pod annotation writer for component descriptors.
//!
//! Writes `ComponentDescriptor` as JSON to the pod's `angzarr.io/descriptor` annotation.
//! This enables K8s-native topology discovery without event bus dependency.

use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, Patch, PatchParams},
    Client,
};
use serde_json::json;
use tracing::{debug, info, warn};

use crate::proto::ComponentDescriptor;

/// Annotation key for component descriptor.
pub const DESCRIPTOR_ANNOTATION: &str = "angzarr.io/descriptor";

/// Error types for annotation operations.
#[derive(Debug, thiserror::Error)]
pub enum AnnotationError {
    #[error("Kubernetes API error: {0}")]
    KubeError(#[from] kube::Error),

    #[error("JSON serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("Missing environment variable: {0}")]
    MissingEnvVar(String),
}

/// Write a component descriptor to the pod's annotation.
///
/// Uses JSON merge patch to update only the descriptor annotation without
/// affecting other pod metadata.
///
/// # Arguments
/// * `client` - K8s client
/// * `namespace` - Pod namespace
/// * `pod_name` - Pod name
/// * `descriptor` - Component descriptor to write
pub async fn write_descriptor_annotation(
    client: &Client,
    namespace: &str,
    pod_name: &str,
    descriptor: &ComponentDescriptor,
) -> Result<(), AnnotationError> {
    let pods: Api<Pod> = Api::namespaced(client.clone(), namespace);

    // Serialize descriptor to JSON
    let descriptor_json = serde_json::to_string(descriptor)?;

    // Create JSON merge patch for annotation
    let patch = json!({
        "metadata": {
            "annotations": {
                DESCRIPTOR_ANNOTATION: descriptor_json
            }
        }
    });

    debug!(
        pod = %pod_name,
        namespace = %namespace,
        descriptor_name = %descriptor.name,
        "Writing descriptor annotation"
    );

    pods.patch(
        pod_name,
        &PatchParams::default(),
        &Patch::Merge(&patch),
    )
    .await?;

    info!(
        pod = %pod_name,
        namespace = %namespace,
        descriptor_name = %descriptor.name,
        component_type = %descriptor.component_type,
        "Descriptor annotation written"
    );

    Ok(())
}

/// Write descriptor annotation if running in K8s environment.
///
/// Reads POD_NAME and POD_NAMESPACE from environment. If not set (non-K8s environment),
/// logs a debug message and returns Ok.
///
/// # Arguments
/// * `descriptor` - Component descriptor to write
pub async fn write_descriptor_if_k8s(descriptor: &ComponentDescriptor) -> Result<(), AnnotationError> {
    let pod_name = match std::env::var(crate::config::POD_NAME_ENV_VAR) {
        Ok(name) => name,
        Err(_) => {
            debug!("POD_NAME not set, skipping descriptor annotation (non-K8s environment)");
            return Ok(());
        }
    };

    let namespace = std::env::var(crate::config::POD_NAMESPACE_ENV_VAR)
        .or_else(|_| std::env::var(crate::config::NAMESPACE_ENV_VAR))
        .unwrap_or_else(|_| "default".to_string());

    let client = match Client::try_default().await {
        Ok(c) => c,
        Err(e) => {
            warn!(error = %e, "Failed to create K8s client, skipping descriptor annotation");
            return Ok(());
        }
    };

    write_descriptor_annotation(&client, &namespace, &pod_name, descriptor).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_descriptor_serialization() {
        let descriptor = ComponentDescriptor {
            name: "saga-order-fulfillment".to_string(),
            component_type: "saga".to_string(),
            inputs: vec![crate::proto::Target {
                domain: "order".to_string(),
                types: vec!["OrderCompleted".to_string()],
            }],
            outputs: vec![crate::proto::Target {
                domain: "fulfillment".to_string(),
                types: vec!["CreateShipment".to_string()],
            }],
        };

        let json = serde_json::to_string(&descriptor).unwrap();
        assert!(json.contains("saga-order-fulfillment"));
        assert!(json.contains("OrderCompleted"));
        assert!(json.contains("CreateShipment"));
    }
}
