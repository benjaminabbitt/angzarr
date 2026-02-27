//! Cucumber World for acceptance tests.
//!
//! The World holds gRPC clients and test state across scenarios.

use angzarr_client::client::CommandHandlerClient;
use angzarr_client::proto::{CommandBook, CommandResponse, Cover, EventPage, Uuid};
use cucumber::World;
use std::env;
use uuid::Uuid as UuidCrate;

/// Default endpoints for aggregate coordinators.
const DEFAULT_PLAYER_ENDPOINT: &str = "http://localhost:1310";
const DEFAULT_TABLE_ENDPOINT: &str = "http://localhost:1311";
const DEFAULT_HAND_ENDPOINT: &str = "http://localhost:1312";

/// Acceptance test world - shared state across cucumber steps.
///
/// Note: We can't derive Debug because CommandHandlerClient doesn't implement it.
#[derive(World)]
#[world(init = Self::new)]
pub struct AcceptanceWorld {
    /// Player domain client
    pub player_client: Option<CommandHandlerClient>,
    /// Table domain client
    pub table_client: Option<CommandHandlerClient>,
    /// Hand domain client
    pub hand_client: Option<CommandHandlerClient>,

    /// Current aggregate root being tested
    pub current_root: Option<Vec<u8>>,
    /// Current domain being tested
    pub current_domain: Option<String>,

    /// Last command response
    pub last_response: Option<CommandResponse>,
    /// Last error
    pub last_error: Option<String>,

    /// Accumulated events for the current aggregate
    pub events: Vec<EventPage>,
}

impl std::fmt::Debug for AcceptanceWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AcceptanceWorld")
            .field("current_domain", &self.current_domain)
            .field("current_root", &self.current_root)
            .field("event_count", &self.events.len())
            .field("last_error", &self.last_error)
            .finish()
    }
}

impl Default for AcceptanceWorld {
    fn default() -> Self {
        Self {
            player_client: None,
            table_client: None,
            hand_client: None,
            current_root: None,
            current_domain: None,
            last_response: None,
            last_error: None,
            events: Vec::new(),
        }
    }
}

impl AcceptanceWorld {
    /// Create a new world, connecting to services from environment or defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize clients - called from Background step.
    pub async fn connect(&mut self) -> Result<(), String> {
        let player_endpoint =
            env::var("PLAYER_ENDPOINT").unwrap_or_else(|_| DEFAULT_PLAYER_ENDPOINT.to_string());
        let table_endpoint =
            env::var("TABLE_ENDPOINT").unwrap_or_else(|_| DEFAULT_TABLE_ENDPOINT.to_string());
        let hand_endpoint =
            env::var("HAND_ENDPOINT").unwrap_or_else(|_| DEFAULT_HAND_ENDPOINT.to_string());

        tracing::info!("Connecting to player aggregate at {}", player_endpoint);
        self.player_client = Some(
            CommandHandlerClient::connect(&player_endpoint)
                .await
                .map_err(|e| format!("Failed to connect to player: {}", e))?,
        );

        tracing::info!("Connecting to table aggregate at {}", table_endpoint);
        self.table_client = Some(
            CommandHandlerClient::connect(&table_endpoint)
                .await
                .map_err(|e| format!("Failed to connect to table: {}", e))?,
        );

        tracing::info!("Connecting to hand aggregate at {}", hand_endpoint);
        self.hand_client = Some(
            CommandHandlerClient::connect(&hand_endpoint)
                .await
                .map_err(|e| format!("Failed to connect to hand: {}", e))?,
        );

        Ok(())
    }

    /// Get client for a domain.
    pub fn client_for_domain(&self, domain: &str) -> Option<&CommandHandlerClient> {
        match domain {
            "player" => self.player_client.as_ref(),
            "table" => self.table_client.as_ref(),
            "hand" => self.hand_client.as_ref(),
            _ => None,
        }
    }

    /// Generate a unique aggregate root for test isolation.
    pub fn new_aggregate_root(&mut self, domain: &str) -> Vec<u8> {
        let unique_id = format!("test-{}-{}", domain, UuidCrate::new_v4());
        let root = UuidCrate::new_v5(&UuidCrate::NAMESPACE_DNS, unique_id.as_bytes())
            .as_bytes()
            .to_vec();
        self.current_root = Some(root.clone());
        self.current_domain = Some(domain.to_string());
        self.events.clear();
        root
    }

    /// Build a Cover for the current aggregate.
    pub fn current_cover(&self) -> Option<Cover> {
        match (&self.current_domain, &self.current_root) {
            (Some(domain), Some(root)) => Some(Cover {
                domain: domain.clone(),
                root: Some(Uuid {
                    value: root.clone(),
                }),
                ..Default::default()
            }),
            _ => None,
        }
    }

    /// Send a command and store the response.
    pub async fn send_command(&mut self, command_book: CommandBook) -> Result<(), String> {
        let domain = command_book
            .cover
            .as_ref()
            .map(|c| c.domain.clone())
            .unwrap_or_default();

        let client = self
            .client_for_domain(&domain)
            .ok_or_else(|| format!("No client for domain: {}", domain))?;

        // Use handle() which wraps CommandBook in CommandRequest
        match client.handle(command_book).await {
            Ok(response) => {
                // Accumulate events from response
                // CommandResponse has `events: EventBook` field
                if let Some(ref event_book) = response.events {
                    self.events.extend(event_book.pages.clone());
                }
                self.last_response = Some(response);
                self.last_error = None;
                Ok(())
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                self.last_response = None;
                Err(e.to_string())
            }
        }
    }

    /// Check if last command succeeded.
    pub fn command_succeeded(&self) -> bool {
        self.last_response.is_some() && self.last_error.is_none()
    }

    /// Get event count for current aggregate.
    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}
