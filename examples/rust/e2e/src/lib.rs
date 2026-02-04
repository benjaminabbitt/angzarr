//! E2E Test Infrastructure for Angzarr Examples
//!
//! Provides a unified test world that works with both standalone (in-process)
//! and gateway (gRPC) backends. Backend selection via `ANGZARR_TEST_MODE` env var.

pub mod adapters;
pub mod backend;
pub mod coverage;
pub mod projectors;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cucumber::World;
use prost::Message;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use tokio::sync::RwLock;
use uuid::Uuid;

use angzarr::proto::{
    event_page::Sequence, CommandBook, CommandPage, CommandResponse, Cover, EventPage,
    Uuid as ProtoUuid,
};

use backend::Backend;

// Re-export proto types for step definitions
pub use angzarr::proto;
pub use common::proto as examples_proto;

/// Projector database paths in gateway mode
const PROJECTOR_DB_PATH: &str = "/tmp/angzarr/projectors";

// ============================================================================
// E2E Test World
// ============================================================================

/// Main test world struct for E2E acceptance tests.
///
/// Uses a `Backend` abstraction to work with both standalone and gateway modes.
/// Backend selection is via `ANGZARR_TEST_MODE` env var:
/// - `standalone` (default): In-process runtime with SQLite memory storage
/// - `gateway`: Remote gRPC gateway at `ANGZARR_ENDPOINT`
#[derive(World)]
#[world(init = Self::new)]
pub struct E2EWorld {
    /// Unified backend for command execution and event queries
    backend: Arc<dyn Backend>,

    /// Map of human-readable names to aggregate root UUIDs
    pub roots: HashMap<String, Uuid>,

    /// Map of correlation ID aliases to actual correlation IDs
    pub correlation_ids: HashMap<String, String>,

    /// Last command response (for assertion in Then steps)
    pub last_response: Option<CommandResponse>,

    /// Last error (for assertion in Then steps)
    pub last_error: Option<String>,

    /// Last temporal query result (for Then steps)
    pub last_temporal_events: Option<Vec<EventPage>>,

    /// Captured event timestamps per cart alias (for timestamp-based scenarios)
    pub captured_timestamps: HashMap<String, Vec<prost_types::Timestamp>>,

    /// Event count snapshot before a dry-run (for verifying no persistence)
    pub event_count_before_dry_run: Option<usize>,

    /// Recorded events for async validation
    recorded_events: Arc<RwLock<Vec<RecordedEvent>>>,

    /// SQLite pool for web projector (gateway mode)
    web_db: Option<SqlitePool>,

    /// SQLite pool for accounting projector (gateway mode)
    accounting_db: Option<SqlitePool>,

    /// Generic key-value context for multi-step scenarios (PM, saga).
    pub context: HashMap<String, String>,

    /// Speculative client for dry-run of projectors, sagas, PMs.
    speculative: Option<Arc<dyn angzarr::client_traits::SpeculativeClient>>,
}

// cucumber::World derive requires Debug
impl std::fmt::Debug for E2EWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("E2EWorld")
            .field("roots", &self.roots)
            .field("last_error", &self.last_error)
            .finish()
    }
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
    /// Create a new E2E test world with the appropriate backend.
    async fn new() -> Self {
        let result = backend::create_backend().await;

        Self {
            backend: result.backend,
            roots: HashMap::new(),
            correlation_ids: HashMap::new(),
            last_response: None,
            last_error: None,
            last_temporal_events: None,
            captured_timestamps: HashMap::new(),
            event_count_before_dry_run: None,
            recorded_events: Arc::new(RwLock::new(Vec::new())),
            web_db: result.web_db,
            accounting_db: result.accounting_db,
            context: HashMap::new(),
            speculative: result.speculative,
        }
    }

    /// Get the backend (for advanced usage)
    pub fn backend(&self) -> &dyn Backend {
        self.backend.as_ref()
    }

    /// Get the speculative client.
    pub fn speculative(&self) -> &dyn angzarr::client_traits::SpeculativeClient {
        self.speculative
            .as_ref()
            .expect("Speculative client not available")
            .as_ref()
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

    /// Build a command book for sending via the backend.
    ///
    /// Sequence is set to 0 as a placeholder. `execute()` queries the store
    /// for the actual next sequence before sending.
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

        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id,
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: format!("type.examples/{}", type_url),
                    value: command.encode_to_vec(),
                }),
            }],
            saga_origin: None,
        }
    }

    /// Build a command targeting an edition (diverged timeline).
    pub fn build_edition_command<M: Message>(
        &mut self,
        domain: &str,
        root_alias: &str,
        correlation_alias: &str,
        type_url: &str,
        command: &M,
        edition_name: &str,
    ) -> CommandBook {
        let root = self.root(root_alias);
        let correlation_id = self.correlation(correlation_alias);

        CommandBook {
            cover: Some(Cover {
                domain: domain.to_string(),
                root: Some(ProtoUuid {
                    value: root.as_bytes().to_vec(),
                }),
                correlation_id,
                edition: Some(edition_name.to_string().into()),
            }),
            pages: vec![CommandPage {
                sequence: 0,
                command: Some(prost_types::Any {
                    type_url: format!("type.examples/{}", type_url),
                    value: command.encode_to_vec(),
                }),
            }],
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
                correlation_id: correlation_id.to_string(),
                edition: None,
            }),
            pages: vec![CommandPage {
                sequence,
                command: Some(prost_types::Any {
                    type_url: format!("type.examples/{}", type_url),
                    value: command.encode_to_vec(),
                }),
            }],
            saga_origin: None,
        }
    }

    /// Execute a command, auto-filling the sequence from the store.
    ///
    /// Queries the actual event count for the target aggregate and sets
    /// the command's sequence accordingly. This eliminates the need for
    /// callers to track sequences locally.
    pub async fn execute(&mut self, mut command_book: CommandBook) {
        // Auto-fill sequence from store
        if let Some(cover) = &command_book.cover {
            let domain = &cover.domain;
            if let Some(root_proto) = &cover.root {
                if let Ok(root) = Uuid::from_slice(&root_proto.value) {
                    let events = self.query_events(domain, root).await;
                    let next_seq = events.len() as u32;
                    if let Some(page) = command_book.pages.first_mut() {
                        page.sequence = next_seq;
                    }
                }
            }
        }

        self.execute_raw(command_book).await;
    }

    /// Execute a command with the exact sequence as provided.
    ///
    /// Use this for resilience tests that need to send specific (possibly wrong)
    /// sequence numbers to test validation behavior.
    pub async fn execute_raw(&mut self, command_book: CommandBook) {
        match self.backend.execute(command_book).await {
            Ok(response) => {
                self.last_response = Some(response);
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                self.last_response = None;
            }
        }
    }

    /// Query events for a domain/root
    pub async fn query_events(&self, domain: &str, root: Uuid) -> Vec<EventPage> {
        self.backend
            .query_events(domain, root)
            .await
            .unwrap_or_default()
    }

    /// Query all events across domains for a given correlation ID.
    /// Returns (domain, event_type, root) tuples.
    pub async fn query_by_correlation(&self, correlation_id: &str) -> Vec<(String, String, Uuid)> {
        self.backend
            .query_by_correlation(correlation_id)
            .await
            .unwrap_or_default()
    }

    /// Query events at a temporal point and store as last_temporal_events
    pub async fn query_events_temporal(
        &mut self,
        domain: &str,
        root: Uuid,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) {
        match self
            .backend
            .query_events_temporal(domain, root, as_of_sequence, as_of_timestamp)
            .await
        {
            Ok(events) => {
                self.last_temporal_events = Some(events);
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                self.last_temporal_events = None;
            }
        }
    }

    /// Dry-run a command at a temporal point
    pub async fn dry_run(
        &mut self,
        command_book: CommandBook,
        as_of_sequence: Option<u32>,
        as_of_timestamp: Option<&str>,
    ) {
        match self
            .backend
            .dry_run(command_book, as_of_sequence, as_of_timestamp)
            .await
        {
            Ok(response) => {
                self.last_response = Some(response);
                self.last_error = None;
            }
            Err(e) => {
                self.last_error = Some(e.to_string());
                self.last_response = None;
            }
        }
    }

    /// Connect to projector SQLite databases (gateway mode)
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
        Some(Sequence::Force(_)) => None,
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
