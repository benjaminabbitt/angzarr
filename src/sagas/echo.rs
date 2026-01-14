//! Echo saga for testing cross-aggregate flows.

use std::sync::Arc;

use async_trait::async_trait;

use crate::interfaces::saga::{Result, Saga};
use crate::proto::{CommandBook, Cover, EventBook, EventPage};
use prost_types::Any;

/// Saga that echoes each event as a command to a target domain.
///
/// Useful for testing saga command generation and cross-aggregate workflows.
/// For each event received, generates a command with the same payload
/// directed at the configured target domain.
pub struct EchoSaga {
    name: String,
    source_domains: Vec<String>,
    target_domain: String,
}

impl EchoSaga {
    /// Create a new echo saga.
    ///
    /// # Arguments
    /// * `name` - Saga identifier
    /// * `target_domain` - Domain to send generated commands to
    pub fn new(name: impl Into<String>, target_domain: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            source_domains: Vec::new(),
            target_domain: target_domain.into(),
        }
    }

    /// Create an echo saga that listens to specific domains.
    pub fn for_domains(
        name: impl Into<String>,
        source_domains: Vec<String>,
        target_domain: impl Into<String>,
    ) -> Self {
        Self {
            name: name.into(),
            source_domains,
            target_domain: target_domain.into(),
        }
    }

    fn event_to_command(
        &self,
        page: &EventPage,
        source_root: Option<&crate::proto::Uuid>,
    ) -> CommandBook {
        // Convert event type to command type (e.g., "OrderCreated" -> "ProcessOrder")
        let command_type = page
            .event
            .as_ref()
            .map(|e| {
                let type_url = &e.type_url;
                // Simple transformation: append "Command" or derive from event name
                if type_url.ends_with("Created") {
                    type_url.replace("Created", "Process")
                } else if type_url.ends_with("Updated") {
                    type_url.replace("Updated", "Sync")
                } else {
                    format!("{}Command", type_url)
                }
            })
            .unwrap_or_else(|| "UnknownCommand".to_string());

        let command_payload = page
            .event
            .as_ref()
            .map(|e| e.value.clone())
            .unwrap_or_default();

        CommandBook {
            cover: Some(Cover {
                domain: self.target_domain.clone(),
                root: source_root.cloned(),
            }),
            pages: vec![crate::proto::CommandPage {
                sequence: 0,
                synchronous: false,
                command: Some(Any {
                    type_url: command_type,
                    value: command_payload,
                }),
            }],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: false,
            fact: false,
        }
    }
}

#[async_trait]
impl Saga for EchoSaga {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        self.source_domains.clone()
    }

    async fn handle(&self, book: &Arc<EventBook>) -> Result<Vec<CommandBook>> {
        let source_root = book.cover.as_ref().and_then(|c| c.root.as_ref());

        let commands: Vec<CommandBook> = book
            .pages
            .iter()
            .map(|page| self.event_to_command(page, source_root))
            .collect();

        Ok(commands)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{event_page, Uuid as ProtoUuid};

    fn make_event_book(domain: &str, events: Vec<&str>) -> EventBook {
        let root = ProtoUuid {
            value: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
        };

        let pages: Vec<EventPage> = events
            .iter()
            .enumerate()
            .map(|(i, event_type)| EventPage {
                sequence: Some(event_page::Sequence::Num(i as u32)),
                event: Some(Any {
                    type_url: (*event_type).to_string(),
                    value: vec![1, 2, 3],
                }),
                created_at: None,
                synchronous: false,
            })
            .collect();

        EventBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(root),
            }),
            pages,
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        }
    }

    #[tokio::test]
    async fn test_echo_saga_generates_commands() {
        let saga = EchoSaga::new("test_echo", "target_domain");
        let book = Arc::new(make_event_book("orders", vec!["OrderCreated", "ItemAdded"]));

        let commands = saga.handle(&book).await.unwrap();

        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0].cover.as_ref().unwrap().domain, "target_domain");
    }

    #[tokio::test]
    async fn test_echo_saga_transforms_event_types() {
        let saga = EchoSaga::new("test_echo", "notifications");
        let book = Arc::new(make_event_book(
            "orders",
            vec!["OrderCreated", "OrderUpdated", "OrderShipped"],
        ));

        let commands = saga.handle(&book).await.unwrap();

        let command_types: Vec<&str> = commands
            .iter()
            .filter_map(|c| c.pages.first())
            .filter_map(|p| p.command.as_ref())
            .map(|c| c.type_url.as_str())
            .collect();

        assert_eq!(
            command_types,
            vec!["OrderProcess", "OrderSync", "OrderShippedCommand"]
        );
    }

    #[tokio::test]
    async fn test_echo_saga_preserves_root() {
        let saga = EchoSaga::new("test_echo", "target");
        let book = Arc::new(make_event_book("orders", vec!["OrderCreated"]));

        let commands = saga.handle(&book).await.unwrap();

        let source_root = book.cover.as_ref().and_then(|c| c.root.as_ref());
        let target_root = commands[0].cover.as_ref().and_then(|c| c.root.as_ref());

        assert_eq!(source_root, target_root);
    }
}
