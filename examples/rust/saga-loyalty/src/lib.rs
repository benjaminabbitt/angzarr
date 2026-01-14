//! Loyalty Points Saga - Rust Implementation.
//!
//! Listens to TransactionCompleted events and sends AddLoyaltyPoints
//! commands to the customer domain.

use std::sync::Arc;

use angzarr::async_trait::async_trait;
use angzarr::interfaces::saga::{Result, Saga};
use angzarr::proto::{CommandBook, CommandPage, Cover, EventBook};
use prost::Message;

pub mod proto;
pub use proto::{AddLoyaltyPoints, TransactionCompleted, TransactionCreated};

/// Saga that awards loyalty points when transactions complete.
#[derive(Debug)]
pub struct LoyaltyPointsSaga {
    name: String,
}

impl LoyaltyPointsSaga {
    /// Create a new loyalty points saga.
    pub fn new() -> Self {
        Self {
            name: "loyalty_points".to_string(),
        }
    }
}

impl Default for LoyaltyPointsSaga {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Saga for LoyaltyPointsSaga {
    fn name(&self) -> &str {
        &self.name
    }

    fn domains(&self) -> Vec<String> {
        vec!["transaction".to_string()]
    }

    async fn handle(&self, book: &Arc<EventBook>) -> Result<Vec<CommandBook>> {
        let mut commands = Vec::new();

        for page in &book.pages {
            let Some(event) = &page.event else {
                continue;
            };

            // Check if this is a TransactionCompleted event
            if !event.type_url.contains("TransactionCompleted") {
                continue;
            }

            // Decode the event using prost
            let Ok(transaction_completed) = TransactionCompleted::decode(event.value.as_slice())
            else {
                continue;
            };

            let points = transaction_completed.loyalty_points_earned;
            if points <= 0 {
                continue;
            }

            // Get customer_id from the transaction cover
            let customer_id = book.cover.as_ref().and_then(|c| c.root.as_ref()).cloned();

            let Some(customer_uuid) = customer_id else {
                continue;
            };

            let transaction_id = book
                .cover
                .as_ref()
                .and_then(|c| c.root.as_ref())
                .map(|r| hex::encode(&r.value))
                .unwrap_or_default();

            println!(
                "[{}] Awarding {} loyalty points for transaction {}...",
                self.name,
                points,
                &transaction_id[..16.min(transaction_id.len())]
            );

            // Create AddLoyaltyPoints command using prost
            let add_points_cmd = AddLoyaltyPoints {
                points,
                reason: format!("transaction:{}", transaction_id),
            };

            let command = CommandBook {
                cover: Some(Cover {
                    domain: "customer".to_string(),
                    root: Some(customer_uuid),
                }),
                pages: vec![CommandPage {
                    sequence: 0,
                    synchronous: false,
                    command: Some(prost_types::Any {
                        type_url: "type.examples/examples.AddLoyaltyPoints".to_string(),
                        value: add_points_cmd.encode_to_vec(),
                    }),
                }],
                correlation_id: String::new(),
                saga_origin: None,     // TODO: Populate from saga context
                auto_resequence: true, // Saga commands should retry on sequence conflicts
                fact: true,            // Transaction already completed, points should be credited
            };

            commands.push(command);
        }

        Ok(commands)
    }

    fn is_synchronous(&self) -> bool {
        true
    }
}

// Public test methods for cucumber tests
impl LoyaltyPointsSaga {
    /// Process an event and return resulting commands (for testing).
    /// This is a direct implementation that avoids async for easier testing.
    pub fn process_event_public(&self, event: &prost_types::Any) -> Vec<CommandBook> {
        let mut commands = Vec::new();

        // Check if this is a TransactionCompleted event
        if !event.type_url.contains("TransactionCompleted") {
            return commands;
        }

        // Decode the event using prost
        let Ok(transaction_completed) = TransactionCompleted::decode(event.value.as_slice()) else {
            return commands;
        };

        let points = transaction_completed.loyalty_points_earned;
        if points <= 0 {
            return commands;
        }

        // Use a fixed transaction ID for testing
        let transaction_id = "0102030405060708090a0b0c0d0e0f10";

        // Create AddLoyaltyPoints command using prost
        let add_points_cmd = AddLoyaltyPoints {
            points,
            reason: format!("transaction:{}", transaction_id),
        };

        let command = CommandBook {
            cover: Some(Cover {
                domain: "customer".to_string(),
                root: Some(angzarr::proto::Uuid {
                    value: vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
                }),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                synchronous: false,
                command: Some(prost_types::Any {
                    type_url: "type.examples/examples.AddLoyaltyPoints".to_string(),
                    value: add_points_cmd.encode_to_vec(),
                }),
            }],
            correlation_id: String::new(),
            saga_origin: None,
            auto_resequence: true,
            fact: true,
        };

        commands.push(command);
        commands
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_transaction_completed() {
        // Encoded TransactionCompleted with loyalty_points_earned = 42
        let event_bytes = vec![
            0x08, 0x90, 0x4e, // field 1: 10000 (final_total_cents)
            0x12, 0x04, 0x63, 0x61, 0x72, 0x64, // field 2: "card"
            0x18, 0x2a, // field 3: 42 (loyalty_points_earned)
        ];

        let event = TransactionCompleted::decode(event_bytes.as_slice()).unwrap();
        assert_eq!(event.final_total_cents, 10000);
        assert_eq!(event.payment_method, "card");
        assert_eq!(event.loyalty_points_earned, 42);
    }

    #[test]
    fn test_encode_add_loyalty_points() {
        let cmd = AddLoyaltyPoints {
            points: 100,
            reason: "transaction:abc123".to_string(),
        };

        let encoded = cmd.encode_to_vec();
        assert!(!encoded.is_empty());

        // Verify round-trip
        let decoded = AddLoyaltyPoints::decode(encoded.as_slice()).unwrap();
        assert_eq!(decoded.points, 100);
        assert_eq!(decoded.reason, "transaction:abc123");
    }
}
