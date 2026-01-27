//! E2E Test Infrastructure for Angzarr Examples
//!
//! This crate provides the test world and utilities for comprehensive
//! end-to-end testing of the angzarr event sourcing system.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cucumber::World;
use prost::Message;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use tokio::sync::RwLock;
use tonic::transport::Channel;
use uuid::Uuid;

use angzarr::proto::{
    command_gateway_client::CommandGatewayClient, event_page::Sequence,
    event_query_client::EventQueryClient, CommandBook, CommandPage, CommandResponse, Cover,
    EventPage, Query, Uuid as ProtoUuid,
};

// Re-export proto types for step definitions
pub use angzarr::proto;
pub use common::proto as examples_proto;

/// Default gateway port for standalone mode
const DEFAULT_GATEWAY_PORT: u16 = 1350;

/// Projector database paths in standalone mode
const PROJECTOR_DB_PATH: &str = "/tmp/angzarr/projectors";

// ============================================================================
// E2E Test World
// ============================================================================

/// Main test world struct for E2E acceptance tests.
///
/// Manages gateway connections, tracks correlation IDs across scenarios,
/// and provides projector state validation via SQLite.
#[derive(World, Debug)]
#[world(init = Self::new)]
pub struct E2EWorld {
    /// Gateway endpoint URL
    pub gateway_endpoint: String,

    /// Lazy-loaded gateway client
    gateway_client: Option<CommandGatewayClient<Channel>>,

    /// Lazy-loaded query client
    query_client: Option<EventQueryClient<Channel>>,

    /// Map of human-readable names to aggregate root UUIDs
    /// e.g., "ALICE" -> Uuid, "WIDGET" -> Uuid
    pub roots: HashMap<String, Uuid>,

    /// Map of correlation ID aliases to actual correlation IDs
    /// e.g., "ORDER-001" -> "550e8400-e29b-41d4-a716-446655440000"
    pub correlation_ids: HashMap<String, String>,

    /// Current sequence number per domain/root combination
    /// Key: "domain:root_uuid"
    sequences: HashMap<String, u32>,

    /// Last command response (for assertion in Then steps)
    pub last_response: Option<CommandResponse>,

    /// Last error (for assertion in Then steps)
    pub last_error: Option<String>,

    /// Recorded events for async validation
    /// Populated by subscribing to the event bus
    recorded_events: Arc<RwLock<Vec<RecordedEvent>>>,

    /// SQLite pool for web projector
    web_db: Option<SqlitePool>,

    /// SQLite pool for accounting projector
    accounting_db: Option<SqlitePool>,
}

/// A recorded event for async validation
#[derive(Debug, Clone)]
pub struct RecordedEvent {
    pub domain: String,
    pub root: Uuid,
    pub event_type: String,
    pub correlation_id: String,
    pub sequence: u32,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl E2EWorld {
    /// Create a new E2E test world
    async fn new() -> Self {
        Self {
            gateway_endpoint: get_gateway_endpoint(),
            gateway_client: None,
            query_client: None,
            roots: HashMap::new(),
            correlation_ids: HashMap::new(),
            sequences: HashMap::new(),
            last_response: None,
            last_error: None,
            recorded_events: Arc::new(RwLock::new(Vec::new())),
            web_db: None,
            accounting_db: None,
        }
    }

    /// Get or create the gateway client
    pub async fn gateway(&mut self) -> &mut CommandGatewayClient<Channel> {
        if self.gateway_client.is_none() {
            let channel = Channel::from_shared(self.gateway_endpoint.clone())
                .expect("Invalid gateway endpoint")
                .connect()
                .await
                .expect("Failed to connect to gateway");
            self.gateway_client = Some(CommandGatewayClient::new(channel));
        }
        self.gateway_client.as_mut().unwrap()
    }

    /// Get or create the query client
    pub async fn query(&mut self) -> &mut EventQueryClient<Channel> {
        if self.query_client.is_none() {
            let channel = Channel::from_shared(self.gateway_endpoint.clone())
                .expect("Invalid query endpoint")
                .connect()
                .await
                .expect("Failed to connect to query service");
            self.query_client = Some(EventQueryClient::new(channel));
        }
        self.query_client.as_mut().unwrap()
    }

    /// Get or create a root UUID for a given alias
    pub fn root(&mut self, alias: &str) -> Uuid {
        *self
            .roots
            .entry(alias.to_string())
            .or_insert_with(Uuid::new_v4)
    }

    /// Get or create a correlation ID for a given alias
    pub fn correlation(&mut self, alias: &str) -> String {
        self.correlation_ids
            .entry(alias.to_string())
            .or_insert_with(|| Uuid::new_v4().to_string())
            .clone()
    }

    /// Get the current sequence for a domain/root combination
    pub fn current_sequence(&self, domain: &str, root: Uuid) -> u32 {
        let key = format!("{}:{}", domain, root);
        *self.sequences.get(&key).unwrap_or(&0)
    }

    /// Increment and get the next sequence for a domain/root combination
    pub fn next_sequence(&mut self, domain: &str, root: Uuid) -> u32 {
        let key = format!("{}:{}", domain, root);
        let seq = self.sequences.entry(key).or_insert(0);
        let current = *seq;
        *seq += 1;
        current
    }

    /// Build a command book for sending to the gateway
    pub fn build_command<M: Message>(
        &mut self,
        domain: &str,
        root_alias: &str,
        correlation_alias: &str,
        type_url: &str,
        command: &M,
    ) -> CommandBook {
        let root = self.root(root_alias);
        let correlation_id = self.correlation(correlation_alias);
        let sequence = self.next_sequence(domain, root);

        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![CommandPage {
                sequence,
                command: Some(prost_types::Any {
                    type_url: format!("type.examples/{}", type_url),
                    value: command.encode_to_vec(),
                }),
            }],
            correlation_id,
            saga_origin: None,
        }
    }

    /// Build a command book with explicit sequence (for testing sequence validation)
    pub fn build_command_with_sequence<M: Message>(
        &self,
        domain: &str,
        root: Uuid,
        correlation_id: &str,
        sequence: u32,
        type_url: &str,
        command: &M,
    ) -> CommandBook {
        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
            }),
            pages: vec![CommandPage {
                sequence,
                command: Some(prost_types::Any {
                    type_url: format!("type.examples/{}", type_url),
                    value: command.encode_to_vec(),
                }),
            }],
            correlation_id: correlation_id.to_string(),
            saga_origin: None,
        }
    }

    /// Execute a command and store the result
    pub async fn execute(&mut self, command_book: CommandBook) {
        let client = self.gateway().await;
        match client.execute(command_book).await {
            Ok(response) => {
                self.last_response = Some(response.into_inner());
                self.last_error = None;
            }
            Err(status) => {
                self.last_error = Some(status.message().to_string());
                self.last_response = None;
            }
        }
    }

    /// Query events for a domain/root
    pub async fn query_events(&mut self, domain: &str, root: Uuid) -> Vec<EventPage> {
        let query = Query {
            domain: domain.to_string(),
            root: Some(ProtoUuid {
                value: root.as_bytes().to_vec(),
            }),
            selection: None, // None means all events
            correlation_id: String::new(),
        };

        let client = self.query().await;
        match client.get_event_book(query).await {
            Ok(response) => response.into_inner().pages,
            Err(_) => vec![],
        }
    }

    /// Connect to projector SQLite databases
    pub async fn connect_projector_dbs(&mut self) -> Result<(), sqlx::Error> {
        let web_path = format!("{}/web.db", PROJECTOR_DB_PATH);
        let accounting_path = format!("{}/accounting.db", PROJECTOR_DB_PATH);

        if std::path::Path::new(&web_path).exists() {
            let options = SqliteConnectOptions::new()
                .filename(&web_path)
                .read_only(true);
            self.web_db = Some(SqlitePool::connect_with(options).await?);
        }

        if std::path::Path::new(&accounting_path).exists() {
            let options = SqliteConnectOptions::new()
                .filename(&accounting_path)
                .read_only(true);
            self.accounting_db = Some(SqlitePool::connect_with(options).await?);
        }

        Ok(())
    }

    /// Wait for events with timeout
    pub async fn wait_for_events(
        &self,
        timeout: Duration,
        predicate: impl Fn(&[RecordedEvent]) -> bool,
    ) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            {
                let events = self.recorded_events.read().await;
                if predicate(&events) {
                    return true;
                }
            }
            if Instant::now() > deadline {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Record an event (called by event bus subscriber)
    pub async fn record_event(&self, event: RecordedEvent) {
        self.recorded_events.write().await.push(event);
    }

    /// Get recorded events
    pub async fn get_recorded_events(&self) -> Vec<RecordedEvent> {
        self.recorded_events.read().await.clone()
    }

    /// Clear recorded events
    pub async fn clear_recorded_events(&self) {
        self.recorded_events.write().await.clear();
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Get the gateway endpoint from environment or use default
fn get_gateway_endpoint() -> String {
    if let Ok(endpoint) = std::env::var("ANGZARR_ENDPOINT") {
        return endpoint;
    }
    let host = std::env::var("ANGZARR_HOST").unwrap_or_else(|_| "localhost".to_string());
    let port: u16 = std::env::var("ANGZARR_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_GATEWAY_PORT);
    format!("http://{}:{}", host, port)
}

/// Extract event type from a protobuf Any type_url
pub fn extract_event_type(event: &prost_types::Any) -> String {
    event
        .type_url
        .rsplit('/')
        .next()
        .unwrap_or(&event.type_url)
        .to_string()
}

/// Extract sequence number from an event page
/// Returns None for forced events (which bypass sequence validation)
pub fn extract_sequence(page: &EventPage) -> Option<u32> {
    match &page.sequence {
        Some(Sequence::Num(n)) => Some(*n),
        Some(Sequence::Force(_)) => None, // Forced events don't have a sequence
        None => Some(0),
    }
}

/// Extract sequence number, defaulting to 0 for forced events
pub fn extract_sequence_or_zero(page: &EventPage) -> u32 {
    extract_sequence(page).unwrap_or(0)
}

/// Convert ProtoUuid to Uuid
pub fn proto_uuid_to_uuid(proto: &ProtoUuid) -> Option<Uuid> {
    Uuid::from_slice(&proto.value).ok()
}

/// Convert Uuid to ProtoUuid
pub fn uuid_to_proto(uuid: Uuid) -> ProtoUuid {
    ProtoUuid {
        value: uuid.as_bytes().to_vec(),
    }
}

// ============================================================================
// Projector Query Structs
// ============================================================================

/// Order projection from web projector
#[derive(Debug, sqlx::FromRow)]
pub struct OrderProjection {
    pub order_id: String,
    pub customer_id: String,
    pub status: String,
    pub subtotal_cents: i64,
    pub discount_cents: i64,
    pub total_cents: i64,
    pub loyalty_points_used: i32,
    pub loyalty_points_earned: i32,
}

/// Ledger entry from accounting projector
#[derive(Debug, sqlx::FromRow)]
pub struct LedgerEntry {
    pub id: i64,
    pub order_id: String,
    pub entry_type: String,
    pub amount_cents: i64,
    pub created_at: String,
}

/// Loyalty balance from accounting projector
#[derive(Debug, sqlx::FromRow)]
pub struct LoyaltyBalance {
    pub customer_id: String,
    pub current_points: i32,
    pub lifetime_points: i32,
}

// ============================================================================
// Projector Query Methods
// ============================================================================

impl E2EWorld {
    /// Query order projection from web projector
    pub async fn query_order_projection(&self, order_id: &str) -> Option<OrderProjection> {
        let pool = self.web_db.as_ref()?;
        sqlx::query_as::<_, OrderProjection>(
            "SELECT order_id, customer_id, status, subtotal_cents, discount_cents,
                    total_cents, loyalty_points_used, loyalty_points_earned
             FROM customer_orders WHERE order_id = ?",
        )
        .bind(order_id)
        .fetch_optional(pool)
        .await
        .ok()?
    }

    /// Query ledger entries for an order from accounting projector
    pub async fn query_ledger_entries(&self, order_id: &str) -> Vec<LedgerEntry> {
        let Some(pool) = self.accounting_db.as_ref() else {
            return vec![];
        };
        sqlx::query_as::<_, LedgerEntry>(
            "SELECT id, order_id, entry_type, amount_cents, created_at
             FROM accounting_ledger WHERE order_id = ? ORDER BY created_at",
        )
        .bind(order_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default()
    }

    /// Query loyalty balance from accounting projector
    pub async fn query_loyalty_balance(&self, customer_id: &str) -> Option<LoyaltyBalance> {
        let pool = self.accounting_db.as_ref()?;
        sqlx::query_as::<_, LoyaltyBalance>(
            "SELECT customer_id, current_points, lifetime_points
             FROM loyalty_balance WHERE customer_id = ?",
        )
        .bind(customer_id)
        .fetch_optional(pool)
        .await
        .ok()?
    }

    /// Wait for projector state with timeout
    pub async fn wait_for_order_projection(
        &self,
        order_id: &str,
        timeout: Duration,
    ) -> Option<OrderProjection> {
        let deadline = Instant::now() + timeout;
        loop {
            if let Some(projection) = self.query_order_projection(order_id).await {
                return Some(projection);
            }
            if Instant::now() > deadline {
                return None;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }
}

// ============================================================================
// Test Assertions
// ============================================================================

/// Assert that the last response contains an event of the given type
pub fn assert_response_has_event(world: &E2EWorld, event_type: &str) {
    let response = world
        .last_response
        .as_ref()
        .expect("No response from last command");
    let events = response.events.as_ref().expect("No events in response");
    assert!(
        !events.pages.is_empty(),
        "Expected events but got empty pages"
    );

    let found = events.pages.iter().any(|page| {
        page.event
            .as_ref()
            .map(|e| extract_event_type(e).contains(event_type))
            .unwrap_or(false)
    });

    assert!(
        found,
        "Expected event type '{}' not found in response",
        event_type
    );
}

/// Assert that the last command failed with a specific status
pub fn assert_command_failed(world: &E2EWorld, expected_status: &str) {
    assert!(
        world.last_response.is_none(),
        "Expected command to fail but got success"
    );
    let error = world.last_error.as_ref().expect("No error message");
    assert!(
        error
            .to_lowercase()
            .contains(&expected_status.to_lowercase()),
        "Expected error containing '{}', got '{}'",
        expected_status,
        error
    );
}

/// Assert that events have contiguous sequences (ignoring forced events)
pub fn assert_contiguous_sequences(events: &[EventPage]) {
    let mut sequences: Vec<u32> = events.iter().filter_map(extract_sequence).collect();
    sequences.sort();

    for (i, seq) in sequences.iter().enumerate() {
        assert_eq!(
            *seq, i as u32,
            "Sequence gap detected: expected {}, got {}",
            i, seq
        );
    }
}
