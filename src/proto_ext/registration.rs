//! Component registration utilities.
//!
//! Functions for building registration commands and deriving component UUIDs.

use crate::orchestration::correlation::ANGZARR_UUID_NAMESPACE;
use crate::proto::{
    CommandBook, CommandPage, ComponentDescriptor, Cover, MergeStrategy, RegisterComponent, Uuid,
};

use angzarr_client::proto_ext::constants::{META_ANGZARR_DOMAIN, REGISTER_COMPONENT_TYPE_URL};

/// Derive a deterministic UUID from a component name.
///
/// Uses the angzarr namespace to ensure consistent root UUIDs across registrations.
pub fn component_name_to_uuid(name: &str) -> uuid::Uuid {
    uuid::Uuid::new_v5(&ANGZARR_UUID_NAMESPACE, name.as_bytes())
}

/// Build registration commands for component descriptors.
///
/// Returns a list of CommandBooks, one per descriptor, targeting the _angzarr
/// meta aggregate with root UUID derived from component name.
pub fn build_registration_commands(
    descriptors: &[ComponentDescriptor],
    pod_id: &str,
) -> Vec<CommandBook> {
    use prost::Message;

    descriptors
        .iter()
        .map(|descriptor| {
            let root_uuid = component_name_to_uuid(&descriptor.name);
            let cmd = RegisterComponent {
                component: Some(descriptor.clone()),
                pod_id: pod_id.to_string(),
            };

            let mut buf = Vec::new();
            cmd.encode(&mut buf).expect("encode RegisterComponent");

            CommandBook {
                cover: Some(Cover {
                    domain: META_ANGZARR_DOMAIN.to_string(),
                    root: Some(Uuid {
                        value: root_uuid.as_bytes().to_vec(),
                    }),
                    correlation_id: format!("registration-{}", descriptor.name),
                    edition: None,
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    command: Some(prost_types::Any {
                        type_url: REGISTER_COMPONENT_TYPE_URL.to_string(),
                        value: buf,
                    }),
                    merge_strategy: MergeStrategy::MergeCommutative as i32,
                    external_payload: None,
                }],
                saga_origin: None,
            }
        })
        .collect()
}

/// Get the current pod ID for component registration.
///
/// In K8s: uses POD_NAME environment variable.
/// Locally: uses hostname or "standalone".
pub fn get_pod_id() -> String {
    std::env::var("POD_NAME").unwrap_or_else(|_| "standalone".to_string())
}
