//! Placeholder business logic for testing without external services.
//!
//! Provides a simple echo-style business logic that creates events
//! from commands without actual domain logic.

use std::collections::HashSet;

use async_trait::async_trait;
use prost_types::Timestamp;
use tracing::info;

use crate::interfaces::business_client::{BusinessLogicClient, Result};
use crate::proto::{
    business_response, event_page::Sequence, BusinessResponse, ContextualCommand, Cover, EventBook,
    EventPage,
};

/// Placeholder business logic that echoes commands as events.
///
/// For each command received, creates a corresponding event.
/// Useful for testing the infrastructure without real business logic.
pub struct PlaceholderBusinessLogic {
    domains: HashSet<String>,
}

impl PlaceholderBusinessLogic {
    /// Create a new placeholder business logic.
    pub fn new(domains: Vec<String>) -> Self {
        Self {
            domains: domains.into_iter().collect(),
        }
    }

    /// Create with default test domains.
    pub fn with_defaults() -> Self {
        Self::new(vec![
            "orders".to_string(),
            "inventory".to_string(),
            "customers".to_string(),
        ])
    }
}

impl Default for PlaceholderBusinessLogic {
    fn default() -> Self {
        Self::with_defaults()
    }
}

#[async_trait]
impl BusinessLogicClient for PlaceholderBusinessLogic {
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<BusinessResponse> {
        let command_book = cmd.command.as_ref();
        let prior_events = cmd.events.as_ref();

        // Determine next sequence number
        let next_sequence = prior_events
            .map(|e| {
                e.pages
                    .iter()
                    .filter_map(|p| match &p.sequence {
                        Some(Sequence::Num(n)) => Some(*n),
                        _ => None,
                    })
                    .max()
                    .map(|n| n + 1)
                    .unwrap_or(0)
            })
            .unwrap_or(0);

        // Get cover from command
        let cover = command_book
            .and_then(|c| c.cover.clone())
            .unwrap_or_else(|| Cover {
                domain: domain.to_string(),
                root: None,
            });

        // Transform each command into an event
        let events: Vec<EventPage> = command_book
            .map(|cb| {
                cb.pages
                    .iter()
                    .enumerate()
                    .map(|(i, cmd_page)| {
                        let event_type = cmd_page
                            .command
                            .as_ref()
                            .map(|c| {
                                // Transform command type to event type
                                // e.g., "CreateOrder" -> "OrderCreated"
                                transform_command_to_event_type(&c.type_url)
                            })
                            .unwrap_or_else(|| "UnknownEvent".to_string());

                        info!(
                            domain = %domain,
                            sequence = next_sequence + i as u32,
                            event_type = %event_type,
                            "Placeholder: creating event from command"
                        );

                        EventPage {
                            sequence: Some(Sequence::Num(next_sequence + i as u32)),
                            created_at: Some(Timestamp {
                                seconds: chrono::Utc::now().timestamp(),
                                nanos: 0,
                            }),
                            event: Some(prost_types::Any {
                                type_url: format!("type.googleapis.com/{}", event_type),
                                value: cmd_page
                                    .command
                                    .as_ref()
                                    .map(|c| c.value.clone())
                                    .unwrap_or_default(),
                            }),
                            synchronous: cmd_page.synchronous,
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();

        Ok(BusinessResponse {
            result: Some(business_response::Result::Events(EventBook {
                cover: Some(cover),
                snapshot: None,
                pages: events,
                correlation_id: String::new(),
                snapshot_state: None,
            })),
        })
    }

    fn has_domain(&self, domain: &str) -> bool {
        self.domains.contains(domain)
    }

    fn domains(&self) -> Vec<String> {
        self.domains.iter().cloned().collect()
    }
}

/// Transform a command type URL to an event type.
///
/// Examples:
/// - "CreateOrder" -> "OrderCreated"
/// - "AddItem" -> "ItemAdded"
/// - "UpdateCustomer" -> "CustomerUpdated"
fn transform_command_to_event_type(type_url: &str) -> String {
    // Extract the type name from the URL
    let type_name = type_url
        .rsplit('/')
        .next()
        .unwrap_or(type_url)
        .rsplit('.')
        .next()
        .unwrap_or(type_url);

    // Common verb transformations
    let transformations = [
        ("Create", "Created"),
        ("Add", "Added"),
        ("Update", "Updated"),
        ("Delete", "Deleted"),
        ("Remove", "Removed"),
        ("Start", "Started"),
        ("Stop", "Stopped"),
        ("Complete", "Completed"),
        ("Cancel", "Cancelled"),
        ("Submit", "Submitted"),
        ("Approve", "Approved"),
        ("Reject", "Rejected"),
    ];

    for (prefix, suffix) in &transformations {
        if let Some(rest) = type_name.strip_prefix(prefix) {
            return format!("{}{}", rest, suffix);
        }
    }

    // Default: append "Processed"
    format!("{}Processed", type_name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proto::{CommandBook, CommandPage, Uuid as ProtoUuid};

    #[test]
    fn test_transform_command_to_event_type() {
        assert_eq!(
            transform_command_to_event_type("CreateOrder"),
            "OrderCreated"
        );
        assert_eq!(transform_command_to_event_type("AddItem"), "ItemAdded");
        assert_eq!(
            transform_command_to_event_type("UpdateCustomer"),
            "CustomerUpdated"
        );
        assert_eq!(
            transform_command_to_event_type("DeleteProduct"),
            "ProductDeleted"
        );
        assert_eq!(
            transform_command_to_event_type("type.googleapis.com/CreateOrder"),
            "OrderCreated"
        );
        assert_eq!(
            transform_command_to_event_type("UnknownCommand"),
            "UnknownCommandProcessed"
        );
    }

    #[test]
    fn test_transform_all_verb_types() {
        assert_eq!(transform_command_to_event_type("RemoveItem"), "ItemRemoved");
        assert_eq!(
            transform_command_to_event_type("StartProcess"),
            "ProcessStarted"
        );
        assert_eq!(
            transform_command_to_event_type("StopProcess"),
            "ProcessStopped"
        );
        assert_eq!(
            transform_command_to_event_type("CompleteTask"),
            "TaskCompleted"
        );
        assert_eq!(
            transform_command_to_event_type("CancelOrder"),
            "OrderCancelled"
        );
        assert_eq!(
            transform_command_to_event_type("SubmitForm"),
            "FormSubmitted"
        );
        assert_eq!(
            transform_command_to_event_type("ApproveRequest"),
            "RequestApproved"
        );
        assert_eq!(
            transform_command_to_event_type("RejectClaim"),
            "ClaimRejected"
        );
    }

    #[test]
    fn test_new_creates_with_specified_domains() {
        let logic =
            PlaceholderBusinessLogic::new(vec!["orders".to_string(), "customers".to_string()]);
        assert!(logic.has_domain("orders"));
        assert!(logic.has_domain("customers"));
        assert!(!logic.has_domain("inventory"));
    }

    #[test]
    fn test_with_defaults_has_standard_domains() {
        let logic = PlaceholderBusinessLogic::with_defaults();
        assert!(logic.has_domain("orders"));
        assert!(logic.has_domain("inventory"));
        assert!(logic.has_domain("customers"));
    }

    #[test]
    fn test_default_creates_with_defaults() {
        let logic = PlaceholderBusinessLogic::default();
        assert!(logic.has_domain("orders"));
    }

    #[test]
    fn test_domains_returns_all_domains() {
        let logic = PlaceholderBusinessLogic::new(vec!["a".to_string(), "b".to_string()]);
        let domains = logic.domains();
        assert_eq!(domains.len(), 2);
        assert!(domains.contains(&"a".to_string()));
        assert!(domains.contains(&"b".to_string()));
    }

    #[tokio::test]
    async fn test_handle_creates_event_from_command() {
        let logic = PlaceholderBusinessLogic::with_defaults();
        let root = uuid::Uuid::new_v4();

        let cmd = ContextualCommand {
            events: None,
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: "orders".to_string(),
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    command: Some(prost_types::Any {
                        type_url: "CreateOrder".to_string(),
                        value: vec![1, 2, 3],
                    }),
                    synchronous: false,
                }],
                correlation_id: String::new(),
                saga_origin: None,
                auto_resequence: false,
                fact: false,
            }),
        };

        let response = logic.handle("orders", cmd).await.unwrap();
        let result = match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events"),
        };

        assert_eq!(result.pages.len(), 1);
        let event = &result.pages[0];
        assert!(matches!(event.sequence, Some(Sequence::Num(0))));
        assert!(event.event.is_some());
        assert!(event
            .event
            .as_ref()
            .unwrap()
            .type_url
            .contains("OrderCreated"));
    }

    #[tokio::test]
    async fn test_handle_with_prior_events_increments_sequence() {
        let logic = PlaceholderBusinessLogic::with_defaults();
        let root = uuid::Uuid::new_v4();

        let prior = EventBook {
            cover: Some(Cover {
                domain: "orders".to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![
                EventPage {
                    sequence: Some(Sequence::Num(0)),
                    event: None,
                    created_at: None,
                    synchronous: false,
                },
                EventPage {
                    sequence: Some(Sequence::Num(1)),
                    event: None,
                    created_at: None,
                    synchronous: false,
                },
            ],
            snapshot: None,
            correlation_id: String::new(),
            snapshot_state: None,
        };

        let cmd = ContextualCommand {
            events: Some(prior),
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: "orders".to_string(),
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    command: Some(prost_types::Any {
                        type_url: "AddItem".to_string(),
                        value: vec![],
                    }),
                    synchronous: false,
                }],
                correlation_id: String::new(),
                saga_origin: None,
                auto_resequence: false,
                fact: false,
            }),
        };

        let response = logic.handle("orders", cmd).await.unwrap();
        let result = match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events"),
        };

        assert_eq!(result.pages.len(), 1);
        assert!(matches!(result.pages[0].sequence, Some(Sequence::Num(2))));
    }

    #[tokio::test]
    async fn test_handle_empty_command_returns_empty_events() {
        let logic = PlaceholderBusinessLogic::with_defaults();

        let cmd = ContextualCommand {
            events: None,
            command: None,
        };

        let response = logic.handle("orders", cmd).await.unwrap();
        let result = match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events"),
        };

        assert!(result.pages.is_empty());
    }

    #[tokio::test]
    async fn test_handle_preserves_synchronous_flag() {
        let logic = PlaceholderBusinessLogic::with_defaults();
        let root = uuid::Uuid::new_v4();

        let cmd = ContextualCommand {
            events: None,
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: "orders".to_string(),
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    command: Some(prost_types::Any {
                        type_url: "CreateOrder".to_string(),
                        value: vec![],
                    }),
                    synchronous: true,
                }],
                correlation_id: String::new(),
                saga_origin: None,
                auto_resequence: false,
                fact: false,
            }),
        };

        let response = logic.handle("orders", cmd).await.unwrap();
        let result = match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events"),
        };

        assert!(result.pages[0].synchronous);
    }

    #[tokio::test]
    async fn test_handle_multiple_commands() {
        let logic = PlaceholderBusinessLogic::with_defaults();
        let root = uuid::Uuid::new_v4();

        let cmd = ContextualCommand {
            events: None,
            command: Some(CommandBook {
                cover: Some(Cover {
                    domain: "orders".to_string(),
                    root: Some(ProtoUuid {
                        value: root.as_bytes().to_vec(),
                    }),
                }),
                pages: vec![
                    CommandPage {
                        sequence: 0,
                        command: Some(prost_types::Any {
                            type_url: "CreateOrder".to_string(),
                            value: vec![],
                        }),
                        synchronous: false,
                    },
                    CommandPage {
                        sequence: 1,
                        command: Some(prost_types::Any {
                            type_url: "AddItem".to_string(),
                            value: vec![],
                        }),
                        synchronous: false,
                    },
                    CommandPage {
                        sequence: 2,
                        command: Some(prost_types::Any {
                            type_url: "AddItem".to_string(),
                            value: vec![],
                        }),
                        synchronous: false,
                    },
                ],
                correlation_id: String::new(),
                saga_origin: None,
                auto_resequence: false,
                fact: false,
            }),
        };

        let response = logic.handle("orders", cmd).await.unwrap();
        let result = match response.result {
            Some(business_response::Result::Events(events)) => events,
            _ => panic!("Expected events"),
        };

        assert_eq!(result.pages.len(), 3);
        assert!(matches!(result.pages[0].sequence, Some(Sequence::Num(0))));
        assert!(matches!(result.pages[1].sequence, Some(Sequence::Num(1))));
        assert!(matches!(result.pages[2].sequence, Some(Sequence::Num(2))));
    }
}
