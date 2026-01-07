//! Placeholder business logic for testing without external services.
//!
//! Provides a simple echo-style business logic that creates events
//! from commands without actual domain logic.

use std::collections::HashSet;

use async_trait::async_trait;
use prost_types::Timestamp;
use tracing::info;

use crate::interfaces::business_client::{BusinessLogicClient, Result};
use crate::proto::{event_page::Sequence, ContextualCommand, Cover, EventBook, EventPage};

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
    async fn handle(&self, domain: &str, cmd: ContextualCommand) -> Result<EventBook> {
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

        Ok(EventBook {
            cover: Some(cover),
            snapshot: None,
            pages: events,
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
}
