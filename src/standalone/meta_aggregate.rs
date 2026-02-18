//! Meta aggregate handler for the `_angzarr` infrastructure domain.
//!
//! Handles component registration commands and emits registration events.
//! This is a built-in aggregate that's automatically registered by the runtime.
//!
//! The aggregate simply translates commands to events - no state, no validation.

use prost::Message;
use prost_types::{Any, Timestamp};
use std::time::SystemTime;
use tonic::Status;

use crate::proto::{
    ComponentDescriptor, ComponentRegistered, ContextualCommand, Cover, EventBook, EventPage,
    RegisterComponent, Target,
};
use crate::proto_ext::{COMPONENT_REGISTERED_TYPE_URL, META_ANGZARR_DOMAIN};
use crate::standalone::AggregateHandler;
use crate::validation;

// Re-export for convenience
pub use crate::proto_ext::META_ANGZARR_DOMAIN as META_DOMAIN;

/// Meta aggregate handler for component registration.
///
/// Simply translates `RegisterComponent` commands into `ComponentRegistered` events.
/// No state, no validation - pure command→event translation.
pub struct MetaAggregateHandler;

impl MetaAggregateHandler {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MetaAggregateHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl AggregateHandler for MetaAggregateHandler {
    fn descriptor(&self) -> ComponentDescriptor {
        ComponentDescriptor {
            name: META_ANGZARR_DOMAIN.to_string(),
            component_type: "aggregate".to_string(),
            inputs: vec![Target {
                domain: META_ANGZARR_DOMAIN.to_string(),
                types: vec!["RegisterComponent".to_string()],
            }],
        }
    }

    async fn handle(&self, ctx: ContextualCommand) -> Result<EventBook, Status> {
        let command_book = ctx
            .command
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing command"))?;

        let cover = command_book
            .cover
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("missing cover"))?;

        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap();

        // Direct 1:1 translation: RegisterComponent → ComponentRegistered
        let pages = command_book
            .pages
            .iter()
            .map(|page| {
                let any = page
                    .command
                    .as_ref()
                    .ok_or_else(|| Status::invalid_argument("missing command in page"))?;

                let cmd = RegisterComponent::decode(&any.value[..])
                    .map_err(|e| Status::invalid_argument(format!("decode error: {e}")))?;

                // Validate component descriptor
                if let Some(ref component) = cmd.component {
                    validation::validate_component_name(&component.name)?;
                    for input in &component.inputs {
                        validation::validate_domain(&input.domain)?;
                    }
                }

                let event = ComponentRegistered {
                    component: cmd.component,
                    pod_id: cmd.pod_id,
                    registered_at: Some(Timestamp {
                        seconds: now.as_secs() as i64,
                        nanos: now.subsec_nanos() as i32,
                    }),
                };

                let mut buf = Vec::new();
                event
                    .encode(&mut buf)
                    .map_err(|e| Status::internal(format!("encode error: {e}")))?;

                Ok(EventPage {
                    sequence: Some(crate::proto::event_page::Sequence::Force(true)),
                    created_at: Some(Timestamp {
                        seconds: now.as_secs() as i64,
                        nanos: now.subsec_nanos() as i32,
                    }),
                    event: Some(Any {
                        type_url: COMPONENT_REGISTERED_TYPE_URL.to_string(),
                        value: buf,
                    }),
                    external_payload: None,
                })
            })
            .collect::<Result<Vec<_>, Status>>()?;

        Ok(EventBook {
            cover: Some(Cover {
                domain: META_ANGZARR_DOMAIN.to_string(),
                root: cover.root.clone(),
                correlation_id: cover.correlation_id.clone(),
                edition: cover.edition.clone(),
            }),
            pages,
            ..Default::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandBook, CommandPage, MergeStrategy, Uuid as ProtoUuid};
    use crate::proto_ext::{component_name_to_uuid, REGISTER_COMPONENT_TYPE_URL};

    fn make_register_command(name: &str, pod_id: &str) -> ContextualCommand {
        let descriptor = ComponentDescriptor {
            name: name.to_string(),
            component_type: "aggregate".to_string(),
            inputs: vec![],
        };

        let cmd = RegisterComponent {
            component: Some(descriptor),
            pod_id: pod_id.to_string(),
        };

        let mut buf = Vec::new();
        cmd.encode(&mut buf).unwrap();

        let root_uuid = component_name_to_uuid(name);

        ContextualCommand {
            events: Some(EventBook::default()),
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: META_DOMAIN.to_string(),
                    root: Some(ProtoUuid {
                        value: root_uuid.as_bytes().to_vec(),
                    }),
                    correlation_id: "test-correlation".to_string(),
                    edition: None,
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    command: Some(Any {
                        type_url: REGISTER_COMPONENT_TYPE_URL.to_string(),
                        value: buf,
                    }),
                    merge_strategy: MergeStrategy::MergeCommutative as i32,
                    external_payload: None,
                }],
                saga_origin: None,
            }),
        }
    }

    #[tokio::test]
    async fn test_register_component_emits_event() {
        let handler = MetaAggregateHandler::new();
        let ctx = make_register_command("order", "pod-123");

        let result = handler.handle(ctx).await.unwrap();

        assert_eq!(result.pages.len(), 1);
        let page = &result.pages[0];
        let event = page.event.as_ref().unwrap();
        assert_eq!(event.type_url, COMPONENT_REGISTERED_TYPE_URL);

        let registered = ComponentRegistered::decode(&event.value[..]).unwrap();
        assert_eq!(registered.component.unwrap().name, "order");
        assert_eq!(registered.pod_id, "pod-123");
        assert!(registered.registered_at.is_some());
    }

    #[tokio::test]
    async fn test_component_name_to_uuid_deterministic() {
        let uuid1 = component_name_to_uuid("order");
        let uuid2 = component_name_to_uuid("order");
        let uuid3 = component_name_to_uuid("inventory");

        assert_eq!(uuid1, uuid2);
        assert_ne!(uuid1, uuid3);
    }
}
